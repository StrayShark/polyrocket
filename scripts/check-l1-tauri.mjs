#!/usr/bin/env node
/**
 * L1 ↔ Tauri command guard (v0.26a).
 *
 * Enforces: every L1 wrapper in `src/ipc.ts` that calls a
 * **sidecar** IPC method MUST have a corresponding Tauri
 * command in `src-tauri/src/commands/sidecar.rs` AND that
 * command MUST be registered in `src-tauri/src/lib.rs` via
 * `tauri::generate_handler!`.
 *
 * Why this exists: between v0.19a and v0.25a, we hit the
 * "wire format but no Tauri command" issue FIVE times
 * (v0.19b → v0.20b back-fill, v0.20a → v0.20b back-fill,
 * v0.23a → v0.23b back-fill, v0.25a → v0.25b back-fill,
 * and one more). This guard would have caught 4 of those 5.
 *
 * Scope: this script ONLY checks L1 wrappers that call
 * sidecar methods (the methods in the `SidecarMethod` enum
 * in `src-tauri/src/domain/lab/sidecar.rs`). Other modules
 * (wallet, market, signal, bet, copy, pnl, llm, etc.) have
 * their own commands and IPCs and are out of scope.
 *
 * Usage: `node scripts/check-l1-tauri.mjs` (or
 * `node scripts/check-doc-sync.mjs` which calls this).
 *
 * Exit codes:
 *   0 — all L1 sidecar wrappers have matching Tauri commands
 *   1 — at least one mismatch (L1 wrapper has no
 *       Tauri command, OR Tauri command is not
 *       registered in lib.rs)
 */

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..');

const ipcPath = resolve(repoRoot, 'src/ipc.ts');
const sidecarPath = resolve(repoRoot, 'src-tauri/src/commands/sidecar.rs');
const libPath = resolve(repoRoot, 'src-tauri/src/lib.rs');
const sidecarDomainPath = resolve(
  repoRoot,
  'src-tauri/src/domain/lab/sidecar.rs',
);

/**
 * Extract the `SidecarMethod` enum from the Rust domain.
 * The enum lists all known sidecar method names. We use
 * this as the allow-list for L1 wrappers.
 *
 * Pattern:
 *   pub enum SidecarMethod {
 *     Ping,
 *     Predict,
 *     ...
 *     AutoPromoteIfBetter,
 *   }
 *
 * We also extract the stringified variants from
 * `as_str()` (e.g. `SidecarMethod::Ping => "ping"`).
 */
function extractSidecarMethods(src) {
  const methods = new Set();
  // Look for as_str match arms: `SidecarMethod::X => "y"`
  const re = /SidecarMethod::(\w+)\s*=>\s*"([^"]+)"/g;
  let m;
  while ((m = re.exec(src)) !== null) {
    methods.add(m[2]);
  }
  return methods;
}

/**
 * Extract L1 wrappers from `src/ipc.ts`.
 *
 * Pattern: `export const X = (...) => invoke<...>('METHOD', ...)`
 *
 * Implementation: split the source on `export const` boundaries
 * and match each wrapper's body independently. This avoids
 * the regex-spanning-multiple-wrappers bug that happens with
 * a single non-greedy `.*?` regex when one wrapper's body is
 * followed by another wrapper that has its own `invoke`.
 */
function extractL1Wrappers(src) {
  const wrappers = [];
  // Split on `export const ` (preserving the boundary).
  // The first chunk (before any `export const`) is the
  // file header; we ignore it.
  const chunks = src.split(/export\s+const\s+/);
  for (let i = 1; i < chunks.length; i++) {
    const chunk = chunks[i];
    // chunk starts with the wrapper name, then "= ... => ... invoke<...>('METHOD', ...)"
    const nameMatch = chunk.match(/^(\w+)\s*=/);
    if (!nameMatch) continue;
    const wrapper = nameMatch[1];
    // Within this chunk, find the first `invoke<...>('METHOD', ...)`
    const invokeMatch = chunk.match(/invoke<[^>]+>\(\s*'([^']+)'/);
    if (!invokeMatch) continue;
    wrappers.push({ wrapper, method: invokeMatch[1] });
  }
  return wrappers;
}

/**
 * Extract Tauri commands from `src-tauri/src/commands/sidecar.rs`.
 */
function extractSidecarCommands(src) {
  const commands = new Set();
  const re = /#\[tauri::command\][\s\S]*?pub\s+(?:async\s+)?fn\s+(\w+)/g;
  let m;
  while ((m = re.exec(src)) !== null) {
    commands.add(m[1]);
  }
  return commands;
}

/**
 * Extract registered commands from `src-tauri/src/lib.rs`.
 */
function extractRegisteredSidecarCommands(src) {
  const registered = new Set();
  const blockMatch = src.match(/tauri::generate_handler!\[([\s\S]*?)\]/);
  if (!blockMatch) return registered;
  const block = blockMatch[1];
  const re = /commands::sidecar::(\w+)/g;
  let m;
  while ((m = re.exec(block)) !== null) {
    registered.add(m[1]);
  }
  return registered;
}

function main() {
  const ipcSrc = readFileSync(ipcPath, 'utf8');
  const sidecarSrc = readFileSync(sidecarPath, 'utf8');
  const libSrc = readFileSync(libPath, 'utf8');
  const sidecarDomainSrc = readFileSync(sidecarDomainPath, 'utf8');

  const sidecarMethods = extractSidecarMethods(sidecarDomainSrc);
  const allWrappers = extractL1Wrappers(ipcSrc);
  // Filter to sidecar-related wrappers only
  const wrappers = allWrappers.filter((w) => sidecarMethods.has(w.method));
  const sidecarCommands = extractSidecarCommands(sidecarSrc);
  const registeredCommands = extractRegisteredSidecarCommands(libSrc);

  const errors = [];

  for (const { wrapper, method } of wrappers) {
    const expectedCommand = method;
    if (!sidecarCommands.has(expectedCommand)) {
      errors.push(
        `L1 wrapper "${wrapper}" calls invoke('${method}', ...)` +
        ` but no Tauri command "${expectedCommand}" exists in` +
        ` src-tauri/src/commands/sidecar.rs`
      );
    } else if (!registeredCommands.has(expectedCommand)) {
      errors.push(
        `L1 wrapper "${wrapper}" calls invoke('${method}', ...)` +
        ` and command "${expectedCommand}" exists in sidecar.rs,` +
        ` but it's NOT registered in` +
        ` src-tauri/src/lib.rs::tauri::generate_handler!`
      );
    }
  }

  if (errors.length === 0) {
    console.log(
      `✓ L1↔Tauri OK (${wrappers.length} sidecar L1 wrappers,` +
      ` ${sidecarCommands.size} sidecar commands,` +
      ` ${registeredCommands.size} registered,` +
      ` ${allWrappers.length - wrappers.length} non-sidecar skipped)`
    );
    process.exit(0);
  }

  console.error('❌ L1↔Tauri mismatch (sidecar methods)');
  for (const e of errors) {
    console.error(`   - ${e}`);
  }
  console.error('');
  console.error(
    '   see docs/polyrocket-v0.25-final.md "Pattern observation"' +
    ' for context'
  );
  process.exit(1);
}

main();
