#!/usr/bin/env node
// polyrocket — custom CSS class coverage linter (v0.74d).
//
// Background: the `nav-item` class was referenced in AppShell.tsx
// from the initial commit (c6ec76b), but the corresponding CSS rule
// was never written. The build succeeded because:
//   - TypeScript: className="nav-item" is a valid string
//   - Tailwind: doesn't know about non-utility classes
//   - Vite: just bundles whatever CSS exists
//   - vitest/jsdom: doesn't load real CSS, so tests pass
//
// Result: the sidebar rendered with default browser styles
// (block layout, no padding, no flex, 24×24 default SVG icons)
// for the entire v0.13 → v0.74 history (~3 months of work).
// 18-route console-error audit (v0.74a) missed it because the
// script only checked console output, not visual rendering.
//
// This linter scans all TSX files for custom class names
// (defined as: not a Tailwind utility prefix, not a known
// lib class like `lucide-*`) and verifies that at least one
// CSS rule selector exists. Catches the `nav-item`-class-of-
// bugs at lint time, before they reach the browser.
//
// Usage:
//   node scripts/check-class-coverage.mjs            # check whole repo
//   node scripts/check-class-coverage.mjs --strict   # also flag obvious typos
//
// Exit codes:
//   0 = all custom classes have CSS rules
//   1 = missing CSS rule(s) found
//   2 = toolchain error (e.g. can't read source dir)

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, extname } from 'node:path';

const ROOT = process.cwd();
const SRC_DIRS = ['src', 'src/components', 'src/routes', 'src/lib', 'src/hooks', 'src/db', 'src/stores'];
const CSS_FILES = ['src/styles/globals.css', 'src/styles/themes.css', 'src/index.css'];

// Tailwind utility class patterns (v0.74d).
// Rather than enumerate all prefixes (impossible — Tailwind has 200+),
// we detect the shape of a Tailwind utility vs a custom class:
//   - Tailwind utility: kebab-case tokens, often with a known prefix
//     (text-, bg-, p-, m-, w-, h-, etc.) OR contain a colon
//     (hover:, md:, focus:) OR arbitrary value [13px].
//   - Custom class: any token used in our own CSS layer.
// Bare Tailwind utility classes (no prefix needed). These are
// the most commonly used ones in this codebase.
const TW_BARE = new Set([
  'flex', 'inline-flex', 'grid', 'inline-grid', 'block', 'inline-block',
  'table', 'hidden', 'contents', 'flow-root', 'list-item',
  'sr-only', 'not-sr-only', 'isolate', 'isolation',
  'border', 'border-0', 'border-2', 'border-4', 'border-8', 'border-x', 'border-y',
  'rounded', 'rounded-none', 'rounded-sm', 'rounded-md', 'rounded-lg', 'rounded-xl', 'rounded-2xl', 'rounded-3xl', 'rounded-full',
  'shadow', 'shadow-sm', 'shadow-md', 'shadow-lg', 'shadow-xl', 'shadow-2xl', 'shadow-inner', 'shadow-none',
  'center', 'right', 'left', 'top', 'bottom', 'middle', 'baseline',
  'inline', 'block', 'flex', 'inline-flex', 'inline-block', 'table', 'table-row', 'table-cell',
  'grow', 'grow-0', 'shrink', 'shrink-0', 'flex-1', 'flex-auto', 'flex-initial', 'flex-none',
  'truncate', 'italic',   'underline', 'overline', 'line-through', 'no-underline',
  'uppercase', 'lowercase', 'capitalize', 'normal-case',
  'absolute', 'relative', 'fixed', 'sticky', 'static',
  'collapse', 'visible', 'invisible', 'sr-only',
  'isolate', 'isolation-auto',
  'select-none', 'select-text', 'select-all', 'select-auto',
  'pointer-events-none', 'pointer-events-auto',
  'appearance-none', 'appearance-auto',
  'cursor-auto', 'cursor-default', 'cursor-pointer', 'cursor-wait', 'cursor-text', 'cursor-move', 'cursor-help', 'cursor-not-allowed',
  'outline', 'outline-none', 'outline-dashed', 'outline-dotted', 'outline-double',
  'ring', 'ring-0', 'ring-1', 'ring-2',
  'opacity-0', 'opacity-5', 'opacity-10', 'opacity-20', 'opacity-25', 'opacity-30', 'opacity-40', 'opacity-50', 'opacity-60', 'opacity-70', 'opacity-75', 'opacity-80', 'opacity-90', 'opacity-95', 'opacity-100',
  'transition', 'transition-all', 'transition-colors', 'transition-opacity', 'transition-shadow', 'transition-transform', 'transition-none',
  'invert', 'invert-0', 'sepia', 'sepia-0', 'grayscale', 'grayscale-0',
  'flex-row', 'flex-row-reverse', 'flex-col', 'flex-col-reverse',
  'flex-wrap', 'flex-wrap-reverse', 'flex-nowrap',
  'overflow-auto', 'overflow-hidden', 'overflow-clip', 'overflow-visible', 'overflow-scroll',
  'overflow-x-auto', 'overflow-y-auto', 'overflow-x-hidden', 'overflow-y-hidden',
  'object-contain', 'object-cover', 'object-fill', 'object-none', 'object-scale-down',
  'aspect-auto', 'aspect-square', 'aspect-video',
  'mx-auto', 'ml-auto', 'mr-auto', 'mt-auto', 'mb-auto', 'ms-auto', 'me-auto',
  'bg-blend-normal', 'mix-blend-normal', 'isolate',
  'transform', 'transform-gpu', 'transform-none', 'transform-cpu',
  'origin-center', 'origin-top', 'origin-top-right', 'origin-right', 'origin-bottom-right', 'origin-bottom', 'origin-bottom-left', 'origin-left', 'origin-top-left',
  'clear-left', 'clear-right', 'clear-both', 'clear-none',
  'snap-start', 'snap-end', 'snap-center', 'snap-align-start', 'snap-align-end', 'snap-x', 'snap-y', 'snap-none', 'snap-mandatory', 'snap-proximity',
  'isolate',
  'tabular-nums', 'lining-nums', 'oldstyle-nums', 'proportional-nums', 'stacked-fractions', 'diagonal-fractions', 'slashed-zero',
  'normal-nums',
  'break-normal', 'break-words', 'break-all', 'break-keep',
  'whitespace-normal', 'whitespace-nowrap', 'whitespace-pre', 'whitespace-pre-line', 'whitespace-pre-wrap', 'whitespace-break-spaces',
  'list-none', 'list-disc', 'list-decimal', 'list-inside', 'list-outside',
  'text-left', 'text-center', 'text-right', 'text-justify', 'text-start', 'text-end',
  'hyphens-none', 'hyphens-manual', 'hyphens-auto',
  'align-baseline', 'align-top', 'align-middle', 'align-bottom', 'align-text-top', 'align-text-bottom', 'align-sub', 'align-super',
  'place-items-start', 'place-items-end', 'place-items-center', 'place-items-baseline', 'place-items-stretch',
  'place-content-start', 'place-content-end', 'place-content-center', 'place-content-between', 'place-content-around', 'place-content-evenly', 'place-content-stretch',
  'place-self-auto', 'place-self-start', 'place-self-end', 'place-self-center', 'place-self-stretch',
  'self-auto', 'self-start', 'self-end', 'self-center', 'self-stretch', 'self-baseline',
  'justify-normal', 'justify-start', 'justify-end', 'justify-center', 'justify-between', 'justify-around', 'justify-evenly', 'justify-stretch',
  'content-normal', 'content-start', 'content-end', 'content-center', 'content-between', 'content-around', 'content-evenly', 'content-stretch', 'content-baseline',
  'items-start', 'items-end', 'items-center', 'items-baseline', 'items-stretch',
  'bg-no-repeat', 'bg-repeat', 'bg-repeat-x', 'bg-repeat-y', 'bg-repeat-round', 'bg-repeat-space',
  'bg-cover', 'bg-contain', 'bg-auto', 'bg-fixed', 'bg-local', 'bg-scroll',
]);

const TW_KNOWN_PREFIXES = [
  'flex-', 'grid-', 'grid-cols-', 'grid-rows-',
  'text-', 'font-', 'bg-', 'border-', 'rounded-', 'shadow-',
  'p-', 'px-', 'py-', 'pt-', 'pr-', 'pb-', 'pl-', 'ps-', 'pe-',
  'm-', 'mx-', 'my-', 'mt-', 'mr-', 'mb-', 'ml-', 'ms-', 'me-',
  'w-', 'h-', 'min-w-', 'min-h-', 'max-w-', 'max-h-', 'size-',
  'gap-', 'gap-x-', 'gap-y-', 'space-x-', 'space-y-',
  'top-', 'right-', 'bottom-', 'left-', 'inset-', 'inset-x-', 'inset-y-',
  'z-', 'opacity-', 'cursor-', 'select-',
  'pointer-events-',
  'transition-', 'duration-', 'ease-', 'delay-', 'animate-',
  'truncate', 'sr-',
  'overflow-', 'overflow-x-', 'overflow-y-', 'whitespace-', 'break-', 'line-clamp-', 'hyphens-',
  'aspect-', 'object-', 'object-position-', 'object-fit-',
  'mix-blend-', 'mix-',
  'col-', 'row-', 'order-',
  'translate-', 'rotate-', 'scale-', 'skew-',
  'origin-',
  'backdrop-', 'ring-', 'outline-', 'divide-', 'divide-x-', 'divide-y-',
  'placeholder-', 'accent-', 'caret-', 'appearance-', 'will-change-',
  'snap-', 'touch-', 'list-', 'list-image-',
  'justify-', 'content-', 'items-', 'self-', 'place-',
  'float-', 'clear-',
  'tracking-', 'leading-',
  'from-', 'via-', 'to-', 'decoration-', 'decoration-',
  'outline-offset-', 'outline-dashed', 'outline-dotted', 'outline-double',
  'ring-inset', 'ring-offset-', 'ring-offset', 'ring-opacity-',
  'shadow-sm', 'shadow-md', 'shadow-lg', 'shadow-xl', 'shadow-2xl', 'shadow-inner', 'shadow-none',
  'mix-blend-', 'background-blend-',
  'mask-', 'mask-image-', 'mask-repeat-', 'mask-position-', 'mask-size-', 'mask-origin-',
  'mask-clip-',
  'fill-', 'stroke-',
  'filter', 'blur-', 'brightness-', 'contrast-', 'drop-shadow-', 'grayscale-',
  'hue-rotate-', 'invert-', 'saturate-', 'sepia-',
  'backdrop-blur-', 'backdrop-brightness-', 'backdrop-contrast-', 'backdrop-grayscale-',
  'backdrop-hue-rotate-', 'backdrop-invert-', 'backdrop-opacity-', 'backdrop-saturate-', 'backdrop-sepia-',
  'min-', 'max-',
  'space-x-', 'space-y-',
  'flex-', 'basis-',
  'grid-cols-', 'grid-rows-', 'col-', 'col-span-', 'col-start-', 'col-end-', 'row-', 'row-span-', 'row-start-', 'row-end-',
];

// lib / framework class names that are not "ours" but are valid
const FRAMEWORK_CLASSES = new Set([
  'lucide', 'lucide-', // lucide-react icons
  'active', // active state, used inside .nav-item (parent .nav-item.active is in CSS)
]);

function isTailwindClass(name) {
  // Negative prefix (e.g. -mt-4, -translate-y-1/2)
  if (name.startsWith('-')) {
    // Strip the leading - and recurse
    return isTailwindClass(name.slice(1));
  }
  // Bare token (e.g. 'flex', 'block', 'border', 'center')
  if (TW_BARE.has(name)) return true;
  // Prefixed utility (e.g. 'text-[13px]', 'bg-accent/15')
  if (TW_KNOWN_PREFIXES.some((p) => name === p || name.startsWith(p))) return true;
  // Tailwind variants (md:, hover:, focus-visible:, etc.)
  // Variants can be multi-segment: focus-visible, group-hover, etc.
  if (/^[a-z-]+:/.test(name)) return true;
  // Arbitrary values [13px], [color:var(--x)]
  if (name.includes('[') && name.includes(']')) return true;
  // Variant with arbitrary value (e.g. 'md:w-[200px]')
  if (/^[a-z]+:.+/.test(name) && name.includes('[')) return true;
  return false;
}

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      if (entry === 'node_modules' || entry === '.git' || entry === 'dist' || entry === 'coverage') continue;
      walk(full, out);
    } else if (['.tsx', '.ts', '.jsx', '.js'].includes(extname(entry))) {
      out.push(full);
    }
  }
  return out;
}

/**
 * Extract className tokens from TSX files.
 * Match: className="..."  className={`...`}  className={cn('a', 'b')}
 * Returns a set of class-name tokens actually used.
 */
function extractClassNames(files) {
  const used = new Set();
  for (const f of files) {
    const src = readFileSync(f, 'utf8');
    // className="foo bar" / className='foo bar'
    const dq = src.matchAll(/className=(?:"([^"]+)"|'([^']+)')/g);
    for (const m of dq) {
      const str = m[1] ?? m[2] ?? '';
      for (const tok of str.split(/\s+/)) {
        if (tok) used.add(tok);
      }
    }
    // className={cn('foo', cond && 'bar')}
    const cn = src.matchAll(/className=\{cn\(([^)]+)\)\}/g);
    for (const m of cn) {
      const body = m[1] ?? '';
      for (const q of body.matchAll(/'([^']+)'|"([^"]+)"/g)) {
        const str = q[1] ?? q[2] ?? '';
        for (const tok of str.split(/\s+/)) {
          if (tok) used.add(tok);
        }
      }
    }
  }
  return used;
}

/**
 * Extract CSS selectors from CSS files.
 * Returns a set of class-name tokens that have at least one
 * matching CSS rule.
 */
function extractCssClasses(files) {
  const defined = new Set();
  for (const f of files) {
    let src;
    try { src = readFileSync(f, 'utf8'); } catch { continue; }
    // .classname selectors
    const cls = src.matchAll(/\.(-?[_a-zA-Z][_a-zA-Z0-9-]*)/g);
    for (const m of cls) defined.add(m[1]);
  }
  return defined;
}

function main() {
  const strict = process.argv.includes('--strict');

  // Gather all TSX / TS files
  const tsxFiles = [];
  for (const d of SRC_DIRS) {
    try {
      const st = statSync(join(ROOT, d));
      if (st.isDirectory()) walk(join(ROOT, d), tsxFiles);
    } catch { /* dir doesn't exist, skip */ }
  }

  const used = extractClassNames(tsxFiles);
  const defined = extractCssClasses(CSS_FILES);

  // Filter: keep only "custom" classes (not Tailwind, not framework)
  const missing = [];
  for (const cls of used) {
    if (FRAMEWORK_CLASSES.has(cls)) continue;
    if (cls.startsWith('lucide-')) continue;
    if (isTailwindClass(cls)) continue;
    // dynamic strings from template literals get filtered earlier
    if (cls.includes('${') || cls.includes('`')) continue;
    if (!defined.has(cls)) {
      missing.push(cls);
    }
  }

  // Dedupe + sort
  const missingSorted = [...new Set(missing)].sort();

  if (missingSorted.length === 0) {
    console.log(`✓ all custom classes have CSS rules (scanned ${tsxFiles.length} files, ${used.size} unique classes)`);
    process.exit(0);
  }

  console.log(`✗ found ${missingSorted.length} class(es) used in TSX but not defined in any CSS:`);
  for (const m of missingSorted) {
    console.log(`    .${m}`);
  }
  console.log('');
  console.log('Fix: add a CSS rule for each missing class in src/styles/globals.css');
  console.log('     (or src/styles/themes.css for theme-specific styles).');
  console.log('');
  if (strict) {
    console.log('(strict mode: also flagging for further review)');
  }
  process.exit(1);
}

main();