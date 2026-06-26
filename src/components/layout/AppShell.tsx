import { useEffect } from 'react';
import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { LayoutDashboard, LineChart, Search, RefreshCw, Bell, Settings, Goal, TrendingUp } from 'lucide-react';
import { KbdHelpDialog, useKbdHelpDialog } from '@/components/feedback/KbdHelpDialog';
import { CommandPalette, useCommandPalette } from '@/components/feedback/CommandPalette';
import { SidecarHealthBadge } from '@/components/feedback/SidecarHealthBadge';
import { useKeyboardNav, useNavBindings } from '@/lib/keyboard-nav';
import { buildPaletteCommands, isPaletteTrigger } from '@/lib/command-palette';
import { isSeeded, syncMarkets, seedDemoData, listWallets, getWalletBalance } from '@/ipc';
import { useQueryClient } from '@tanstack/react-query';
import { useT } from '@/lib/i18n';
import { cn } from '@/lib/cn';

// v0.123 —— 仅 football 范围。侧栏缩减为 2 个主要
// 路由(Dashboard + Markets)。已移除:signals、copy、pnl、
// lab、trade、bankroll、history、wallets、audit、brief、help、
// llm-perf、llm-mgmt。所有被移除的路由仍可通过直接 URL
// 访问以便诊断,但不再出现在侧栏中。
// v0.126 —— 新增 Arb Board 导航项。
const PRIMARY_NAV = [
  { to: '/dashboard', icon: LayoutDashboard, i18nKey: 'nav.dashboard' },
  { to: '/markets', icon: LineChart, i18nKey: 'nav.markets' },
  { to: '/arb-board', icon: TrendingUp, i18nKey: 'nav.arb_board' },
];


/**
 * `AppShell` —— 整 app 的 layout 外壳。
 *
 * **结构**：
 *   - 顶 bar —— logo + breadcrumb + global actions (Sync / Sidecar badge / Bell / Settings)
 *   - 左 sidebar —— primary nav (7 routes) + category nav (1 category, v0.119 football-only)
 *   - 主区域 —— `<Outlet>`（当前 route content）
 *
 * **键盘快捷键**：
 *   - `Cmd+K` / `Ctrl+K` —— 打开 `CommandPalette`（搜索 / 跳转 / 跑命令）
 *   - `?` —— 打开 `KbdHelpDialog`（快捷键 help）
 *   - `1`-`7` —— 跳到 primary nav 第 N 项
 *
 * **数据流**：
 *   1. mount 时 `isSeeded()` 检查 DB 是否首次启动 → 跳到 `/welcome`
 *   2. 各 IPC（sync / recompute / purge）通过 `CommandPalette` 触发
 *   3. Toast 容器在 sidebar 底部
 *
 * **i18n**：nav 项的 label 都走 i18n key（`nav.dashboard` 等）。
 */
export function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { t } = useT();
  const segments = location.pathname.split('/').filter(Boolean);
  const route = segments[0] ?? 'dashboard';
  // 特殊情况:/markets/:id → "Market Detail" 而不是 "Markets"
  let pageKey = `page.${route.replace(/-/g, '_')}`;
  if (route === 'markets' && segments.length > 1) {
    pageKey = 'page.market_detail';
  }
  const pretty = t(pageKey) !== `?${pageKey}?`
    ? t(pageKey)
    : route.charAt(0).toUpperCase() + route.slice(1);

  // v0.8d —— 键盘导航(g d / g m / ? / Esc)
  const kbdHelp = useKbdHelpDialog();
  // v0.9c —— 命令面板(Cmd+K)
  const palette = useCommandPalette();

  const bindings = useNavBindings({
    onOpenHelp: kbdHelp.openDialog,
    onOpenSearch: () => {
      // 搜索框位于顶栏;通过 DOM 选择器聚焦它。
      // (未来:等顶栏拆分为独立组件后,改成正式的 ref。)
      const input = document.querySelector<HTMLInputElement>('input[type="search"], input[placeholder*="Search" i]');
      input?.focus();
    },
    onCloseDialog: kbdHelp.closeDialog,
  });
  const { pendingPrefix } = useKeyboardNav(bindings);

  // v0.123 —— 侧栏底部读取真实钱包 + USDC 余额,
  // 来源为 .env 中的 L2 凭据(PM CLOB balance-allowance)。
  // 两条 query 都在 focus 时重新拉取,使用户在 paper trade
  // 或切换钱包地址后能立刻看到最新余额。
  const walletsQuery = useQuery({
    queryKey: ['wallets'],
    queryFn: () => listWallets(),
    staleTime: 30_000,
  });
  const balanceQuery = useQuery({
    queryKey: ['wallet-balance'],
    queryFn: () => getWalletBalance(),
    staleTime: 30_000,
    refetchInterval: 60_000,
  });
  const wallet = walletsQuery.data?.[0];
  const balanceText = balanceQuery.data?.ok
    ? `${balanceQuery.data.balance_usdc.toFixed(2)} USDC`
    : '— USDC';
  const walletShort = wallet?.address
    ? `${wallet.address.slice(0, 6)}…${wallet.address.slice(-3)}`
    : '0x000…000';

  // v0.9c —— palette 命令
  const paletteCommands = buildPaletteCommands({
    onNavigate: navigate,
    onOpenHelp: kbdHelp.openDialog,
    onSyncMarkets: async () => {
      await syncMarkets();
      queryClient.invalidateQueries({ queryKey: ['markets'] });
    },
onOpenSettings: () => navigate('/settings'),
    onResetDemoData: async () => {
      await seedDemoData(true);
      await isSeeded();  // 触摸以避免 import 被识别为 dead
      queryClient.invalidateQueries();
    },
  });

  // v0.9c —— 全局 Cmd+K / Ctrl+K 监听器
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (isPaletteTrigger(e)) {
        e.preventDefault();
        palette.openPalette();
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [palette]);

  return (
    <div className="flex h-screen">
      {/* 侧边栏 */}
      <aside
        className="w-[220px] shrink-0 flex flex-col border-r"
        style={{ background: 'var(--surface)', borderColor: 'var(--border)' }}
      >
        {/* Logo */}
        <div className="h-12 px-3 flex items-center gap-2 border-b" style={{ borderColor: 'var(--border)' }}>
          <div className="w-6 h-6 rounded-md grid place-items-center" style={{ background: 'var(--accent)' }}>
            <Goal className="w-3.5 h-3.5 text-white" />
          </div>
          <span className="text-body-sm font-semibold tracking-tight" style={{ color: 'var(--fg)' }}>
            polyrocket
          </span>
          <span
            className="ml-auto font-mono text-[10px] px-1.5 py-0.5 rounded border"
            style={{ color: 'var(--muted)', borderColor: 'var(--border)' }}
          >
            v0.1
          </span>
        </div>

        {/* 导航 */}
        <div className="flex-1 overflow-auto py-2">
          <SectionLabel>Workspace</SectionLabel>
          {PRIMARY_NAV.map((n) => (
            <NavItem key={n.to} {...n} t={t} />
          ))}

          <SectionLabel className="mt-4">{t('nav.settings_section')}</SectionLabel>
          <div className="nav-item">
            <Settings className="w-3.5 h-3.5" />
            <span>{t('nav.preferences')}</span>
          </div>
        </div>

        {/* v0.123 —— 钱包底部。通过 `getWalletBalance()`(L2 HMAC)
            从 PM CLOB 读取真实 USDC 余额。
            在查询完成前,或 .env 凭据 / L1 地址缺失时,
            `balanceText` 为 "— USDC"。*/}
        <div className="p-2 border-t" style={{ borderColor: 'var(--border)' }}>
          <div
            className="mt-2 flex items-center gap-2 px-1 py-1 text-[11px]"
            style={{ color: 'var(--muted)' }}
            data-testid="wallet-footer"
            title={balanceQuery.data?.reason || 'PM CLOB balance (L2 HMAC)'}
          >
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ background: balanceQuery.data?.ok ? 'var(--bull)' : 'var(--muted)' }}
            />
            <span className="font-mono">{walletShort}</span>
            <span className="ml-auto">{balanceText}</span>
          </div>
        </div>

      </aside>

      {/* 主区域 */}
      <main className="flex-1 flex flex-col min-w-0">
        {/* 顶部栏 */}
        <header
          className="h-12 px-4 flex items-center gap-3 border-b"
          style={{ background: 'var(--surface)', borderColor: 'var(--border)' }}
        >
          <div className="flex items-center gap-1.5 text-xs">
            <span style={{ color: 'var(--fg)' }}>Dashboard</span>
            <span style={{ color: 'var(--muted)' }}>/</span>
            <span style={{ color: 'var(--muted)' }}>{pretty}</span>
          </div>
          <div className="ml-3 flex-1 max-w-md">
            <div
              className="flex items-center gap-2 h-7 px-2.5 rounded-md text-xs border"
              style={{ background: 'var(--surface-2)', borderColor: 'var(--border)', color: 'var(--muted)' }}
            >
              <Search className="w-3.5 h-3.5" />
              <span>Search matches…</span>
              <span
                className="ml-auto font-mono text-[10px] px-1 py-0.5 rounded border"
                style={{ borderColor: 'var(--border)' }}
              >
                ⌘K
              </span>
            </div>
          </div>
          <div className="ml-auto flex items-center gap-1">
            {/* v0.8d —— 待定组合键指示器(例如 "g…") */}
            {pendingPrefix && (
              <span
                className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface border border-border text-muted"
                aria-live="polite"
              >
                {pendingPrefix}…
              </span>
            )}
            {/* v0.10d —— sidecar 健康徽章 */}
            <SidecarHealthBadge />
            <IconBtn><RefreshCw className="w-3.5 h-3.5" /></IconBtn>
            <IconBtn
              aria-label="Keyboard shortcuts"
              title="Keyboard shortcuts (?)"
              onClick={kbdHelp.openDialog}
            >
              <span className="text-[10px] font-mono">?</span>
            </IconBtn>
            <IconBtn><Bell className="w-3.5 h-3.5" /></IconBtn>
            <IconBtn><Settings className="w-3.5 h-3.5" /></IconBtn>
            <div
              className="w-7 h-7 rounded-full grid place-items-center text-[11px] font-semibold border"
              style={{ background: 'var(--accent)', color: 'white', borderColor: 'var(--border)', opacity: 0.92 }}
            >
              D
            </div>
          </div>
        </header>

        {/* Outlet */}
        <div className="flex-1 overflow-auto">
          <Outlet />
        </div>
      </main>

      {/* v0.8d —— 键盘帮助 dialog(按 `?` 打开) */}
      <KbdHelpDialog bindings={bindings} open={kbdHelp.open} onClose={kbdHelp.closeDialog} />

      {/* v0.9c —— 命令面板(按 Cmd+K 打开) */}
      <CommandPalette
        commands={paletteCommands}
        open={palette.open}
        onClose={palette.closePalette}
      />
    </div>
  );
}

function SectionLabel({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        'px-3 mb-1 text-xs text-muted font-semibold uppercase tracking-caption-uppercase',
        className,
      )}
      style={{ color: 'var(--muted)' }}
    >
      {children}
    </div>
  );
}

function NavItem({ to, icon: Icon, i18nKey, label, count, badge, t }: {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  i18nKey?: string;
  /** 无 i18nKey 时的 fallback。 */
  label?: string;
  count?: string;
  badge?: string;
  t: (key: string, vars?: Record<string, string | number>) => string;
}) {
  const text = i18nKey ? t(i18nKey) : label ?? '';
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cn('nav-item', isActive && 'active')
      }
    >
      <Icon className="w-3.5 h-3.5" />
      <span>{text}</span>
      {count && (
        <span className="ml-auto font-mono text-[10px]" style={{ color: 'var(--muted)' }}>
          {count}
        </span>
      )}
      {badge && (
        <span
          className="ml-auto px-1.5 rounded text-[10px] font-medium"
          style={{ background: 'var(--bull)', color: 'white', opacity: 0.85 }}
        >
          {badge}
        </span>
      )}
    </NavLink>
  );
}

function IconBtn({
  children,
  onClick,
  title,
  ...rest
}: {
  children: React.ReactNode;
  onClick?: () => void;
  title?: string;
  [k: string]: unknown;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      className="w-7 h-7 grid place-items-center rounded-md hover:bg-surface-hover"
      style={{ color: 'var(--fg-secondary)' }}
      {...rest}
    >
      {children}
    </button>
  );
}
