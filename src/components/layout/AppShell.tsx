import { NavLink, Outlet, useLocation } from 'react-router-dom';
import { LayoutDashboard, LineChart, Zap, Copy, BarChart3, FlaskConical, Search, RefreshCw, Bell, Settings, Radar, CircleDot, Crosshair, Landmark, Circle } from 'lucide-react';
import { ThemeSwitcher } from '@/components/theme/ThemeSwitcher';
import { KbdHelpDialog, useKbdHelpDialog } from '@/components/feedback/KbdHelpDialog';
import { useKeyboardNav, useNavBindings } from '@/lib/keyboard-nav';
import { cn } from '@/lib/cn';

const PRIMARY_NAV = [
  { to: '/dashboard', icon: LayoutDashboard, label: 'Dashboard' },
  { to: '/markets', icon: LineChart, label: 'Markets', count: '128' },
  { to: '/signals', icon: Zap, label: 'Signals', badge: '7' },
  { to: '/copy', icon: Copy, label: 'Copy Trading' },
  { to: '/pnl', icon: BarChart3, label: 'P&L' },
  { to: '/lab', icon: FlaskConical, label: 'Model Lab' },
];

const CATEGORY_NAV = [
  { icon: CircleDot, label: 'Football', count: '62' },
  { icon: Crosshair, label: 'CS2', count: '41' },
  { icon: Landmark, label: 'Politics', count: '25' },
];

export function AppShell() {
  const location = useLocation();
  const breadcrumb = location.pathname.split('/').filter(Boolean)[0] ?? 'dashboard';
  const pretty = breadcrumb.charAt(0).toUpperCase() + breadcrumb.slice(1);

  // v0.8d — keyboard navigation (g d / g m / ? / Esc)
  const kbdHelp = useKbdHelpDialog();
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
            <NavItem key={n.to} {...n} />
          ))}

          <SectionLabel className="mt-4">Categories</SectionLabel>
          {CATEGORY_NAV.map((c) => (
            <div key={c.label} className="nav-item">
              <c.icon className="w-3.5 h-3.5" style={{ color: 'var(--muted)' }} />
              <span>{c.label}</span>
              <span className="ml-auto font-mono text-[10px]" style={{ color: 'var(--muted)' }}>
                {c.count}
              </span>
            </div>
          ))}

          <SectionLabel className="mt-4">Settings</SectionLabel>
          <div className="nav-item">
            <Settings className="w-3.5 h-3.5" />
            <span>Preferences</span>
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

function NavItem({ to, icon: Icon, label, count, badge }: {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  count?: string;
  badge?: string;
}) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cn('nav-item', isActive && 'active')
      }
    >
      <Icon className="w-3.5 h-3.5" />
      <span>{label}</span>
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