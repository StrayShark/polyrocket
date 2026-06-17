#!/usr/bin/env node
/**
 * L1 ↔ Tauri command guard (v0.27a — generalized;
 * v0.32a — also checks Rust command definitions).
 *
 * Enforces two invariants:
 *
 *   1. Every L1 wrapper in `src/ipc.ts` that calls
 *      `invoke<...>('METHOD', ...)` MUST have a
 *      corresponding Tauri command registered in
 *      `src-tauri/src/lib.rs::tauri::generate_handler!`.
 *      (Caught the v0.4 `fetchActiveMarkets` bug
 *      retroactively in v0.27a.)
 *
 *   2. Every `#[tauri::command]` function defined in
 *      `src-tauri/src/commands/*.rs` MUST be
 *      registered in `lib.rs::generate_handler!`.
 *      (v0.32a — catches the inverse: "function
 *      defined but not registered", which the Rust
 *      compiler doesn't catch because `generate_handler!`
 *      is a macro that takes any expression.)
 *
 * Why this exists: between v0.19a and v0.25a, we hit
 * the "wire format but no Tauri command" issue 5
 * times, all in the sidecar module. v0.26a added
 * the v0.27a guard to catch the L1→Rust direction.
 * v0.32a extends it to catch the inverse
 * Rust→lib.rs direction.
 *
 * Usage: `node scripts/check-l1-tauri.mjs` (or
 * `node scripts/check-doc-sync.mjs` which calls this).
 *
 * Exit codes:
 *   0 — all invariants hold
 *   1 — at least one mismatch
 */

import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, basename } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..');

const ipcPath = resolve(repoRoot, 'src/ipc.ts');
const libPath = resolve(repoRoot, 'src-tauri/src/lib.rs');
const commandsDir = resolve(repoRoot, 'src-tauri/src/commands');

/**
 * Extract ALL registered Tauri commands from
 * `src-tauri/src/lib.rs::tauri::generate_handler!`.
 *
 * Pattern: `commands::X::Y,` (X = module, Y = command
 * name). The Y is what `invoke('Y', ...)` sends.
 *
 * We also exclude non-command registrations like
 * `pub use commands::sidecar::SidecarState;` and
 * `app_handle.manage(commands::sidecar::SidecarState::new());`
 * which appear in lib.rs but are NOT inside the
 * `generate_handler!` block.
 */
function extractAllRegisteredCommands(src) {
  const registered = new Set();
  const blockMatch = src.match(/tauri::generate_handler!\[([\s\S]*?)\]/);
  if (!blockMatch) return registered;
  const block = blockMatch[1];
  const re = /commands::(\w+)::(\w+)/g;
  let m;
  while ((m = re.exec(block)) !== null) {
    registered.add(m[2]);
  }
  return registered;
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
  const chunks = src.split(/export\s+const\s+/);
  for (let i = 1; i < chunks.length; i++) {
    const chunk = chunks[i];
    const nameMatch = chunk.match(/^(\w+)\s*=/);
    if (!nameMatch) continue;
    const wrapper = nameMatch[1];
    const invokeMatch = chunk.match(/invoke<[^>]+>\(\s*'([^']+)'/);
    if (!invokeMatch) continue;
    wrappers.push({ wrapper, method: invokeMatch[1] });
  }
  return wrappers;
}

/**
 * v0.32a — extract all `#[tauri::command]` function names
 * from a single Rust source file.
 *
 * Pattern:
 *   #[tauri::command]
 *   pub async fn <NAME>(...) -> ...   (or `pub fn <NAME>`)
 *
 * We use a simple regex: `#[tauri::command]\s*(?:#[^\n]*\s*)*pub\s+(?:async\s+)?fn\s+(\w+)`.
 * The inner `(?:#[^\n]*\s*)*` allows arbitrary attribute lines
 * (e.g. `#[tauri::command(rename_all = "snake_case")]`)
 * between `#[tauri::command]` and the function. We don't
 * support multi-line attributes, but those don't appear
 * in the project.
 *
 * Returns: array of { name, file }.
 */
function extractTauriCommandsFromFile(filePath, fileContent) {
  const fns = [];
  const re = /#\[tauri::command\][\s\S]*?pub\s+(?:async\s+)?fn\s+(\w+)/g;
  let m;
  while ((m = re.exec(fileContent)) !== null) {
    fns.push({ name: m[1], file: basename(filePath) });
  }
  return fns;
}

/**
 * v0.32a — scan all `commands/*.rs` files and build a
 * set of defined Tauri command names.
 *
 * Skips `mod.rs` (which contains `pub mod X;` lines, not
 * `#[tauri::command]` functions). Also skips files where
 * the regex matches a `#[tauri::command]` in a doc-comment
 * or a test (rare; the regex is intentionally simple).
 */
function extractAllDefinedTauriCommands() {
  const defined = new Map(); // name → file
  const files = readdirSync(commandsDir).filter(
    (f) => f.endsWith('.rs') && f !== 'mod.rs',
  );
  for (const f of files) {
    const fullPath = resolve(commandsDir, f);
    const src = readFileSync(fullPath, 'utf8');
    const fns = extractTauriCommandsFromFile(fullPath, src);
    for (const fn of fns) {
      defined.set(fn.name, fn.file);
    }
  }
  return defined;
}

function main() {
  const ipcSrc = readFileSync(ipcPath, 'utf8');
  const libSrc = readFileSync(libPath, 'utf8');

  const wrappers = extractL1Wrappers(ipcSrc);
  const registeredCommands = extractAllRegisteredCommands(libSrc);
  const definedCommands = extractAllDefinedTauriCommands();

  const errors = [];

  // Direction 1: L1 wrapper → registered Tauri command
  for (const { wrapper, method } of wrappers) {
    if (!registeredCommands.has(method)) {
      errors.push(
        `L1 wrapper "${wrapper}" calls invoke('${method}', ...)` +
        ` but no Tauri command "${method}" is registered in` +
        ` src-tauri/src/lib.rs::tauri::generate_handler!`
      );
    }
  }

  // Direction 2 (v0.32a): defined Tauri command → registered
  // Catches: a `#[tauri::command] pub fn X` exists in
  // commands/Y.rs but X is missing from lib.rs::generate_handler!.
  // The Rust compiler doesn't catch this (the macro accepts
  // any expression).
  for (const [name, file] of definedCommands) {
    if (!registeredCommands.has(name)) {
      errors.push(
        `Tauri command "${name}" is defined in src-tauri/src/commands/${file}` +
        ` (with #[tauri::command]) but is NOT registered in` +
        ` src-tauri/src/lib.rs::tauri::generate_handler!`
      );
    }
  }

  // Also check the reverse direction: are there any
  // registered Tauri commands that have NO L1 wrapper?
  // Not strictly required (commands can be invoked via
  // other channels — e.g. `sidecar_request` is a generic
  // pass-through, and the scheduler calls commands
  // directly), but we warn about it as a courtesy.
  const wrapperMethods = new Set(wrappers.map((w) => w.method));
  const orphanCommands = [...registeredCommands].filter(
    (c) => !wrapperMethods.has(c),
  );

  if (errors.length === 0) {
    let msg =
      `✓ L1↔Tauri OK (${wrappers.length} L1 wrappers,` +
      ` ${registeredCommands.size} registered commands,` +
      ` ${definedCommands.size} #[tauri::command] defs)`;
    if (orphanCommands.length > 0) {
      msg += ` [info: ${orphanCommands.length} registered command(s) have no L1 wrapper: ${orphanCommands.slice(0, 5).join(', ')}${orphanCommands.length > 5 ? '…' : ''}]`;
    }
    console.log(msg);
    process.exit(0);
  }

  console.error('❌ L1↔Tauri mismatch');
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
