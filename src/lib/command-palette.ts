/**
 * L1 — Command palette registry + fuzzy filter (v0.9c).
 *
 * The palette is opened with `Cmd+K` (or `Ctrl+K` on Win/Linux) and
 * shows a flat list of "commands" — anything the user might want
 * to do, indexed by label, alias, and category.
 *
 * Cmd+K is a different key chord from the `g`-prefix navigation
 * in v0.8d: that one is for route jumping, this one is for
 * "do a thing" (navigate is just one possible action).
 *
 * The fuzzy filter is a tiny substring + case-insensitive scorer,
 * not a real fuzzy library — sufficient for ~50 commands, and
 * zero runtime deps.
 */

export interface PaletteCommand {
  /** Stable id; used as the React key and for highlight tracking. */
  id: string;
  /** Display label, e.g. "Sync markets". */
  label: string;
  /** Free-text aliases the user might type, e.g. ["refresh", "fetch"]. */
  aliases?: string[];
  /** Section name (group), e.g. "Navigate" / "Wallets". */
  category: string;
  /** Optional icon name (lucide-react), looked up by the renderer. */
  icon?: string;
  /** Optional kbd shortcut hint to display on the right side. */
  shortcut?: string[];
  /** The action. */
  action: () => void;
}

/** Default command set — wired to the v0.9c palette. */
export function buildPaletteCommands(opts: {
  onNavigate: (path: string) => void;
  onOpenHelp: () => void;
  onSyncMarkets: () => void;
  onRecomputeSignals: () => void;
  onOpenSettings: () => void;
  onResetDemoData: () => void;
  onPurgeAuditLog: () => void;
}): PaletteCommand[] {
  return [
    // Navigation
    { id: 'nav.dashboard',  label: 'Go to Dashboard',  category: 'Navigate', shortcut: ['g', 'd'], action: () => opts.onNavigate('/dashboard') },
    { id: 'nav.markets',    label: 'Go to Markets',    category: 'Navigate', shortcut: ['g', 'm'], action: () => opts.onNavigate('/markets') },
    { id: 'nav.signals',    label: 'Go to Signals',    category: 'Navigate', shortcut: ['g', 's'], action: () => opts.onNavigate('/signals') },
    { id: 'nav.copy',       label: 'Go to Copy',       category: 'Navigate', shortcut: ['g', 'c'], action: () => opts.onNavigate('/copy') },
    { id: 'nav.pnl',        label: 'Go to P&L',        category: 'Navigate', shortcut: ['g', 'p'], action: () => opts.onNavigate('/pnl') },
    { id: 'nav.history',    label: 'Go to History',    category: 'Navigate', shortcut: ['g', 'h'], action: () => opts.onNavigate('/history') },
    { id: 'nav.wallets',    label: 'Go to Wallets',    category: 'Navigate', shortcut: ['g', 'w'], action: () => opts.onNavigate('/wallets') },
    { id: 'nav.lab',        label: 'Go to Lab',        category: 'Navigate', shortcut: ['g', 'l'], action: () => opts.onNavigate('/lab') },
    { id: 'nav.settings',   label: 'Go to Settings',   category: 'Navigate', action: () => opts.onOpenSettings() },
    { id: 'nav.help',       label: 'Show keyboard shortcuts', aliases: ['shortcuts', 'help me'], category: 'Navigate', shortcut: ['?'], action: () => opts.onOpenHelp() },
    // Actions
    { id: 'act.sync',       label: 'Sync markets',  aliases: ['refresh markets', 'fetch markets'], category: 'Actions', action: opts.onSyncMarkets },
    { id: 'act.recompute',  label: 'Recompute signals', aliases: ['recompute', 're-rank'], category: 'Actions', action: opts.onRecomputeSignals },
    { id: 'act.reset',      label: 'Reset demo data',    aliases: ['re-seed', 'reseed'], category: 'Actions', action: opts.onResetDemoData },
    { id: 'act.purge',      label: 'Purge audit log',    aliases: ['clear audit', 'wipe audit'], category: 'Actions', action: opts.onPurgeAuditLog },
  ];
}

/**
 * Score one command against a query. Returns 0 if no match.
 * Higher = better match.
 *
 *   - exact label match → 100
 *   - label starts with query → 80
 *   - label contains query → 60
 *   - alias exact → 70
 *   - alias contains → 40
 *   - char-overlap → 1..30 (worst-case)
 */
export function scoreCommand(cmd: PaletteCommand, query: string): number {
  if (!query) return 50;  // empty query: keep stable order
  const q = query.toLowerCase();
  const label = cmd.label.toLowerCase();
  if (label === q) return 100;
  if (label.startsWith(q)) return 80;
  if (label.includes(q)) return 60;
  if (cmd.aliases) {
    for (const a of cmd.aliases) {
      const al = a.toLowerCase();
      if (al === q) return 70;
      if (al.includes(q)) return 40;
    }
  }
  // Character-overlap fuzzy: count shared characters in order.
  let qi = 0;
  for (const ch of label) {
    if (qi < q.length && ch === q[qi]) qi++;
  }
  if (qi === q.length) return 30;
  return 0;
}

export function filterCommands(
  commands: PaletteCommand[],
  query: string,
  limit = 10,
): PaletteCommand[] {
  if (!query.trim()) return commands.slice(0, limit);
  const scored = commands
    .map((c) => ({ c, s: scoreCommand(c, query) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s || a.c.label.localeCompare(b.c.label));
  return scored.slice(0, limit).map((x) => x.c);
}

/**
 * Detect Cmd+K (mac) or Ctrl+K (others). Returns true if the event
 * is the trigger. Used in the global keydown listener.
 */
export function isPaletteTrigger(e: KeyboardEvent): boolean {
  return (e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k';
}
