import { useEffect } from 'react';
import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom';
import { LayoutDashboard, LineChart, Zap, Copy, BarChart3, FlaskConical, Search, RefreshCw, Bell, Settings, Radar, CircleDot, Crosshair, Landmark, Circle } from 'lucide-react';
import { ThemeSwitcher } from '@/components/theme/ThemeSwitcher';
import { LocaleSwitcher } from '@/components/i18n/LocaleSwitcher';
import { KbdHelpDialog, useKbdHelpDialog } from '@/components/feedback/KbdHelpDialog';
import { CommandPalette, useCommandPalette } from '@/components/feedback/CommandPalette';
import { SidecarHealthBadge } from '@/components/feedback/SidecarHealthBadge';
import { useKeyboardNav, useNavBindings } from '@/lib/keyboard-nav';
import { buildPaletteCommands, isPaletteTrigger } from '@/lib/command-palette';
import { isSeeded, syncMarkets, recomputeSignals, seedDemoData, purgeAuditLogNow } from '@/ipc';
import { useQueryClient } from '@tanstack/react-query';
import { useT } from '@/lib/i18n';
import { cn } from '@/lib/cn';

const PRIMARY_NAV = [
  { to: '/dashboard', icon: LayoutDashboard, i18nKey: 'nav.dashboard' },
  { to: '/markets', icon: LineChart, i18nKey: 'nav.markets', count: '128' },
  { to: '/signals', icon: Zap, i18nKey: 'nav.signals', badge: '7' },
  { to: '/copy', icon: Copy, i18nKey: 'nav.copy' },
  { to: '/pnl', icon: BarChart3, i18nKey: 'nav.pnl' },
  { to: '/lab', icon: FlaskConical, i18nKey: 'nav.lab' },
];

const CATEGORY_NAV = [
  { icon: CircleDot, label: 'Football', count: '62' },
  { icon: Crosshair, label: 'CS2', count: '41' },
  { icon: Landmark, label: 'Politics', count: '25' },
];

export function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { t } = useT();
  const breadcrumb = location.pathname.split('/').filter(Boolean)[0] ?? 'dashboard';
  const pretty = breadcrumb.charAt(0).toUpperCase() + breadcrumb.slice(1);

  // v0.8d — keyboard navigation (g d / g m / ? / Esc)
  const kbdHelp = useKbdHelpDialog();
  // v0.9c — command palette (Cmd+K)
  const palette = useCommandPalette();

  const bindings = useNavBindings({
    onOpenHelp: kbdHelp.openDialog,
    onOpenSearch: () => {
      // Search box lives in the topbar; focus it via a DOM selector.
      // (Future: hoist this into a proper ref when the topbar is split
      // into its own component.)
      const input = document.querySelector<HTMLInputElement>('input[type="search"], input[placeholder*="Search" i]');
      input?.focus();
    },
    onCloseDialog: kbdHelp.closeDialog,
  });
  const { pendingPrefix } = useKeyboardNav(bindings);

  // v0.9c — palette commands
  const paletteCommands = buildPaletteCommands({
    onNavigate: navigate,
    onOpenHelp: kbdHelp.openDialog,
    onSyncMarkets: async () => {
      await syncMarkets();
      queryClient.invalidateQueries({ queryKey: ['markets'] });
    },
    onRecomputeSignals: async () => {
      await recomputeSignals();
      queryClient.invalidateQueries({ queryKey: ['signals'] });
    },
    onOpenSettings: () => navigate('/settings'),
    onResetDemoData: async () => {
      await seedDemoData(true);
      await isSeeded();  // touch so import isn't dead
      queryClient.invalidateQueries();
    },
    onPurgeAuditLog: async () => {
      const n = await purgeAuditLogNow();
      queryClient.invalidateQueries({ queryKey: ['audit'] });
      // eslint-disable-next-line no-console
      console.log(`purged ${n} audit rows`);
    },
  });

  // v0.9c — global Cmd+K / Ctrl+K listener
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
      {/* Sidebar */}
      <aside
        className="w-[220px] shrink-0 flex flex-col border-r"
        style={{ background: 'var(--surface)', borderColor: 'var(--border)' }}
      >
        {/* Logo */}
        <div className="h-12 px-3 flex items-center gap-2 border-b" style={{ borderColor: 'var(--border)' }}>
          <div className="w-6 h-6 rounded-md grid place-items-center" style={{ background: 'var(--accent)' }}>
            <Radar className="w-3.5 h-3.5 text-white" />
          </div>
          <span className="text-[13px] font-semibold tracking-tight" style={{ color: 'var(--fg)' }}>
            polyrocket
          </span>
          <span
            className="ml-auto font-mono text-[10px] px-1.5 py-0.5 rounded border"
            style={{ color: 'var(--muted)', borderColor: 'var(--border)' }}
          >
            v0.1
          </span>
        </div>

        {/* Nav */}
        <div className="flex-1 overflow-auto py-2">
          <SectionLabel>Workspace</SectionLabel>
          {PRIMARY_NAV.map((n) => (
            <NavItem key={n.to} {...n} t={t} />
          ))}

          <SectionLabel className="mt-4">{t('nav.categories')}</SectionLabel>
          {CATEGORY_NAV.map((c) => (
            <div key={c.label} className="nav-item">
              <c.icon className="w-3.5 h-3.5" style={{ color: 'var(--muted)' }} />
              <span>{c.label}</span>
              <span className="ml-auto font-mono text-[10px]" style={{ color: 'var(--muted)' }}>
                {c.count}
              </span>
            </div>
          ))}

          <SectionLabel className="mt-4">{t('nav.settings_section')}</SectionLabel>
          <div className="nav-item">
            <Settings className="w-3.5 h-3.5" />
            <span>{t('nav.preferences')}</span>
          </div>
          <div className="nav-item">
            <Circle className="w-3.5 h-3.5" />
            <span>API Keys</span>
          </div>
        </div>

        {/* Bottom: theme + wallet */}
        <div className="p-2 border-t" style={{ borderColor: 'var(--border)' }}>
          <div className="px-1 mb-1.5 text-[10px] font-medium uppercase tracking-wider" style={{ color: 'var(--muted)' }}>
            Theme
          </div>
          <ThemeSwitcher />
          <div className="mt-2">
            <LocaleSwitcher />
          </div>
          <div className="mt-2 flex items-center gap-2 px-1 py-1 text-[11px]" style={{ color: 'var(--muted)' }}>
            <span className="w-1.5 h-1.5 rounded-full" style={{ background: 'var(--bull)' }} />
            <span className="font-mono">0x4f…a91</span>
            <span className="ml-auto">12,847 USDC</span>
          </div>
        </div>
      </aside>

      {/* Main */}
      <main className="flex-1 flex flex-col min-w-0">
        {/* TopBar */}
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
              <span>Search markets, signals…</span>
              <span
                className="ml-auto font-mono text-[10px] px-1 py-0.5 rounded border"
                style={{ borderColor: 'var(--border)' }}
              >
                ⌘K
              </span>
            </div>
          </div>
          <div className="ml-auto flex items-center gap-1">
            {/* v0.8d — pending key chord indicator (e.g. "g…") */}
            {pendingPrefix && (
              <span
                className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface border border-border text-muted"
                aria-live="polite"
              >
                {pendingPrefix}…
              </span>
            )}
            {/* v0.10d — sidecar health badge */}
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

      {/* v0.8d — keyboard help dialog (`?` to open) */}
      <KbdHelpDialog bindings={bindings} open={kbdHelp.open} onClose={kbdHelp.closeDialog} />

      {/* v0.9c — command palette (Cmd+K to open) */}
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
      className={cn('px-3 mb-1 text-[10px] font-medium uppercase tracking-wider', className)}
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
  /** Fallback if no i18nKey. */
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