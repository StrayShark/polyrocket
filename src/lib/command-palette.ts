/**
 * L1 — 命令面板注册表 + 模糊过滤（v0.9c）。
 *
 * 通过 `Cmd+K`（在 Win/Linux 上为 `Ctrl+K`）打开面板，
 * 显示一个扁平的「命令」列表 —— 用户可能想做的任何操作，
 * 按 label、alias 和 category 索引。
 *
 * Cmd+K 是与 v0.8d 中 `g` 前缀导航不同的按键组合：
 * 那个用于路由跳转，而这个用于「执行某个操作」
 * （导航只是其中一种可能操作）。
 *
 * 模糊过滤是一个简单的子串 + 不区分大小写的打分器，
 * 并非真正的模糊匹配库 —— 对于约 50 条命令足够，
 * 且无运行时依赖。
 */

export interface PaletteCommand {
  /** 稳定的 id；用作 React key 以及高亮追踪。 */
  id: string;
  /** 显示标签，例如 "Sync markets"。 */
  label: string;
  /** 用户可能输入的文本别名，例如 ["refresh", "fetch"]。 */
  aliases?: string[];
  /** 分组名称（section），例如 "Navigate" / "Wallets"。 */
  category: string;
  /** 可选的图标名（lucide-react），由渲染器解析。 */
  icon?: string;
  /** 可选的快捷键提示，显示在右侧。 */
  shortcut?: string[];
  /** 要执行的动作。 */
  action: () => void;
}

/** 默认命令集合 —— 与 v0.9c 面板绑定。 */
export function buildPaletteCommands(opts: {
  onNavigate: (path: string) => void;
  onOpenHelp: () => void;
  onSyncMarkets: () => void;
  onOpenSettings: () => void;
  onResetDemoData: () => void;
}): PaletteCommand[] {
  return [
    // v0.123 — 仅限 football 范围。已移除 nav.signals / nav.copy /
    // nav.pnl / nav.history / nav.wallets / nav.lab 以及
    // recompute / purge 动作，因为这些界面不再
    // 在侧边栏中暴露。
    { id: 'nav.dashboard',  label: 'Go to Dashboard',  category: 'Navigate', shortcut: ['g', 'd'], action: () => opts.onNavigate('/dashboard') },
    { id: 'nav.markets',    label: 'Go to Matches',    category: 'Navigate', shortcut: ['g', 'm'], action: () => opts.onNavigate('/markets') },
    { id: 'nav.analysis',   label: 'Go to Analysis',   category: 'Navigate', shortcut: ['g', 'a'], action: () => opts.onNavigate('/analysis') },
    { id: 'nav.brief',      label: 'Go to Daily Brief', category: 'Navigate', action: () => opts.onNavigate('/brief') },
    { id: 'nav.audit',      label: 'Go to Audit',       category: 'Navigate', action: () => opts.onNavigate('/audit') },
    { id: 'nav.arb-board',  label: 'Go to Arb Board',   aliases: ['arbitrage', 'arb board'], category: 'Navigate', action: () => opts.onNavigate('/arb-board') },
    { id: 'nav.llm-mgmt',   label: 'Go to LLM Management', aliases: ['llm providers'], category: 'Navigate', action: () => opts.onNavigate('/llm-mgmt') },
    { id: 'nav.llm-perf',   label: 'Go to LLM Performance', aliases: ['llm stats'], category: 'Navigate', action: () => opts.onNavigate('/llm-perf') },
    { id: 'nav.bankroll',   label: 'Go to Bankroll',    category: 'Navigate', action: () => opts.onNavigate('/bankroll') },
    { id: 'nav.settings',   label: 'Go to Settings',   category: 'Navigate', action: () => opts.onOpenSettings() },
    { id: 'nav.help',       label: 'Show keyboard shortcuts', aliases: ['shortcuts', 'help me'], category: 'Navigate', shortcut: ['?'], action: () => opts.onOpenHelp() },
    // 操作
    { id: 'act.sync',       label: 'Sync matches',  aliases: ['refresh matches', 'fetch matches'], category: 'Actions', action: opts.onSyncMarkets },
    { id: 'act.reset',      label: 'Reset demo data',    aliases: ['re-seed', 'reseed'], category: 'Actions', action: opts.onResetDemoData },
  ];
}

/**
 * 对一个命令相对于查询进行评分。
 * 不匹配时返回 0。越高 = 越匹配。
 *
 *   - exact label match → 100
 *   - label starts with query → 80
 *   - label contains query → 60
 *   - alias exact → 70
 *   - alias contains → 40
 *   - char-overlap → 1..30 (worst-case)
 */
export function scoreCommand(cmd: PaletteCommand, query: string): number {
  if (!query) return 50;  // 空查询：保持稳定顺序
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
  // 字符重叠模糊匹配:按顺序统计共享字符数。
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
 * 检测 Cmd+K（mac）或 Ctrl+K（其他平台）。
 * 如果该事件是触发键则返回 true。
 * 用于全局 keydown 监听器。
 */
export function isPaletteTrigger(e: KeyboardEvent): boolean {
  return (e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k';
}
