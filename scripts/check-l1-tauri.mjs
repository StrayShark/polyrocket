#!/usr/bin/env node
/**
 * L1 ↔ Tauri command guard (v0.27a — generalized).
 *
 * Enforces: every L1 wrapper in `src/ipc.ts` that calls
 * `invoke<...>('METHOD', ...)` MUST have a corresponding
 * Tauri command registered in
 * `src-tauri/src/lib.rs::tauri::generate_handler!`.
 *
 * v0.27a — generalized the v0.26a sidecar-only guard to
 * cover ALL modules (wallet, market, signal, bet, copy,
 * pnl, llm, llm_mgmt, polyrocket, scheduler, notification,
 * audit, brief, sidecar, etc.). The approach is the same
 * — extract registered command names from lib.rs and
 * check every L1 wrapper's method name against the set —
 * but we no longer filter by the `SidecarMethod` enum.
 *
 * Why this exists: between v0.19a and v0.25a, we hit the
 * "wire format but no Tauri command" issue 5 times, all
 * in the sidecar module. The same risk exists for other
 * modules. v0.27a extends the v0.26a guard to all
 * modules, catching the same bug class for the rest of
 * the project.
 *
 * Usage: `node scripts/check-l1-tauri.mjs` (or
 * `node scripts/check-doc-sync.mjs` which calls this).
 *
 * Exit codes:
 *   0 — all L1 wrappers have matching Tauri commands
 *   1 — at least one mismatch
 */

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..');

const ipcPath = resolve(repoRoot, 'src/ipc.ts');
const libPath = resolve(repoRoot, 'src-tauri/src/lib.rs');

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

function main() {
  const ipcSrc = readFileSync(ipcPath, 'utf8');
  const libSrc = readFileSync(libPath, 'utf8');

  const wrappers = extractL1Wrappers(ipcSrc);
  const registeredCommands = extractAllRegisteredCommands(libSrc);

  const errors = [];

  for (const { wrapper, method } of wrappers) {
    if (!registeredCommands.has(method)) {
      errors.push(
        `L1 wrapper "${wrapper}" calls invoke('${method}', ...)` +
        ` but no Tauri command "${method}" is registered in` +
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
    let msg = `✓ L1↔Tauri OK (${wrappers.length} L1 wrappers,` +
              ` ${registeredCommands.size} registered commands)`;
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
