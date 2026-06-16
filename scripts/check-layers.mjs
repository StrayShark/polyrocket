#!/usr/bin/env node
/**
 * LayerGuard pre-commit check (governance §1.2).
 *
 * Enforces the strict 5-layer dependency rules declared in
 * docs/overview.md §1.2:
 *
 *     L1 Presentation  →  L2 Application  →  L3 Domain  →  L4 Infrastructure  →  L5 Platform
 *
 * Allowed edges (one-way, downward only):
 *   L1 → L2, L3, L4, L5
 *   L2 → L3, L4, L5
 *   L3 → L4, L5
 *   L4 → L5
 *
 * Disallowed edges (any direction that skips layers or goes upward):
 *   L2 → L1
 *   L3 → L1, L2
 *   L4 → L1, L2, L3
 *   L5 → L1, L2, L3, L4
 *   L3 → L2 (no, wait — that's allowed, see above)
 *
 * Implementation: walks the staged Rust source files under
 * `src-tauri/src/**` and `src-tauri/tests/**` and looks at every
 * `use crate::X` import. Each X is mapped to a layer by its file
 * location (path → layer map below). If any disallowed edge is
 * detected, the script exits non-zero.
 *
 * Notes:
 * - `crate::AppError` / `crate::AppResult` are top-level re-exports
 *   of `crate::infra::error::*` and resolve to L4. They are allowed
 *   from any layer (they're sugar).
 * - The scheduler in L4 imports from L3 (llm_clients); this is the
 *   single intentional cross-layer edge for the runtime bridge. We
 *   allow it by exempting `infra/scheduler/**`.
 * - This script is intentionally path-based (not `cargo metadata`):
 *   fast, no Rust toolchain dep, and easy to read.
 */

import { execSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

// ---------------------------------------------------------------- path → layer
//
// Path → layer mapping. Update this when you add a new top-level
// directory under src-tauri/src/ (e.g. `domain/foo/`, `infra/bar/`).
//
// `level` is the numeric layer (1 = top, 5 = bottom).
// `boundary` is true for files that may skip layers (the cross-layer
// runtime bridge in L4).

function classify(path) {
  // Normalize to forward slashes
  const p = path.replace(/\\/g, '/');
  // Strip prefix
  const rel = p.startsWith('src-tauri/') ? p.slice('src-tauri/'.length) : p;

  if (rel.startsWith('src/bin/')) {
    // dev_smoke / future CLI binaries — treat as L2 (they call L3)
    return { layer: 2, kind: 'bin' };
  }
  if (rel.startsWith('src/commands/')) {
    return { layer: 2, kind: 'commands' };
  }
  if (rel.startsWith('src/domain/')) {
    return { layer: 3, kind: 'domain' };
  }
  if (rel.startsWith('src/infra/scheduler/')) {
    // Runtime bridge: L4 → L3 allowed (calls into llm_clients)
    return { layer: 4, kind: 'infra-scheduler' };
  }
  if (rel.startsWith('src/infra/')) {
    return { layer: 4, kind: 'infra' };
  }
  if (rel.startsWith('src/platform/')) {
    return { layer: 5, kind: 'platform' };
  }
  if (rel.startsWith('src/lib.rs')) {
    // lib.rs is L2 (it wires up the app); imports allowed downward
    return { layer: 2, kind: 'lib' };
  }
  if (rel.startsWith('tests/')) {
    // Integration tests are L2-equivalent (they exercise the public API)
    return { layer: 2, kind: 'test' };
  }
  return { layer: null, kind: 'unknown' };
}

// Resolve a `use crate::X::Y` or `crate::X::Y` to a path under src-tauri/src/
// Best-effort: we look for the longest prefix of `X` that matches a
// known top-level module (commands, domain, infra, platform, llm_clients).
// Anything else (e.g. `crate::AppError`, `crate::Uuid`) we treat as
// a top-level re-export and resolve to L4 (infra::error re-exports).

function resolveImportTarget(importPath) {
  // importPath examples:
  //   "crate::domain::llm::CallOutcome"
  //   "crate::infra::http::new_http_client"
  //   "crate::platform::keyring::set_key"
  //   "crate::AppError"                  (top-level re-export → infra::error)
  //   "polyrocket_lib::domain::llm::X"   (from tests/bin)

  const segments = importPath
    .replace(/^crate::/, '')
    .replace(/^polyrocket_lib::/, '')
    .split('::');

  // Top-level re-exports of AppError/AppResult live in L4
  if (segments[0] === 'AppError' || segments[0] === 'AppResult' || segments[0] === 'AppState') {
    if (segments[0] === 'AppState') return { layer: 4, kind: 're-export-AppState' };
    return { layer: 4, kind: 're-export-AppError' };
  }

  const TOP_LEVEL = ['commands', 'domain', 'infra', 'platform', 'llm_clients', 'polymarket'];
  if (TOP_LEVEL.includes(segments[0])) {
    const layer = { commands: 2, llm_clients: 3, polymarket: 3, domain: 3, infra: 4, platform: 5 }[segments[0]];
    return { layer, kind: segments[0] };
  }

  return { layer: null, kind: 'external-or-unknown' };
}

// ---------------------------------------------------------------- scan

const staged = execSync('git diff --cached --name-only --diff-filter=ACMR')
  .toString()
  .trim()
  .split('\n')
  .filter(Boolean);

// Only scan Rust source files
const rsFiles = staged.filter(
  (f) => (f.startsWith('src-tauri/src/') || f.startsWith('src-tauri/tests/')) && f.endsWith('.rs'),
);

if (rsFiles.length === 0) {
  console.log('✓ no Rust files staged, skipping layer check');
  process.exit(0);
}

const violations = [];

for (const rel of rsFiles) {
  const abs = resolve(ROOT, rel);
  const src = readFileSync(abs, 'utf8');
  const source = classify(rel);
  if (source.layer === null) continue; // unknown — skip (e.g. build.rs)

  // Match `use crate::X` and `crate::X::Y` references.
  // We deliberately don't try to parse Rust — the regex is permissive
  // and skips false positives by also matching on word boundaries.
  const importRe = /\b(?:use\s+)?crate::([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)/g;
  let m;
  while ((m = importRe.exec(src)) !== null) {
    const target = resolveImportTarget(`crate::${m[1]}`);
    if (target.layer === null) continue;

    // Allowed: source.layer >= target.layer (going down or staying)
    // The cross-layer bridge in L4 scheduler → L3 llm_clients is allowed.
    const isSchedulerBridge =
      source.kind === 'infra-scheduler' && target.kind === 'llm_clients';

    if (target.layer < source.layer && !isSchedulerBridge) {
      // Pull a 1-line context for the error message
      const lineNum = src.slice(0, m.index).split('\n').length;
      const lineText = src.split('\n')[lineNum - 1].trim();
      violations.push({
        file: rel,
        line: lineNum,
        text: lineText,
        from: `L${source.layer} (${source.kind})`,
        to: `L${target.layer} (${target.kind})`,
        importPath: `crate::${m[1]}`,
      });
    }
  }
}

if (violations.length > 0) {
  console.error('❌ LayerGuard violation — disallowed layer edges detected:\n');
  for (const v of violations) {
    console.error(`  ${v.file}:${v.line}`);
    console.error(`    ${v.text}`);
    console.error(`    ↑ ${v.from} imports from ${v.to}`);
    console.error(`    import: ${v.importPath}\n`);
  }
  console.error(`${violations.length} violation(s) total.`);
  console.error('See docs/overview.md §1.2 for the allowed layer edges.');
  process.exit(1);
}

console.log(`✓ layer check OK (${rsFiles.length} Rust file(s) scanned)`);
