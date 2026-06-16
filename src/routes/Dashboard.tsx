import { useQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { Wallet, Activity, Target, FlaskConical, CheckCircle2, ExternalLink, Copy, Download, Plus, Filter, RefreshCw } from 'lucide-react';
import { cn } from '@/lib/cn';

// ---------- Types mirror src-tauri/src/commands/* DTOs ----------
type DashboardKpis = {
  total_equity_usdc: string;
  open_pnl_usdc: string;
  win_rate_30d: number;
  brier_score: number;
  active_signals: number;
  open_positions: number;
};

type Signal = {
  id: number;
  market_id: string;
  predicted_prob: number;
  market_prob: number;
  edge: number;
  confidence: number;
  market_question?: string;
  market_slug?: string;
};

type Bet = {
  id: string;
  market_id: string;
  side: string;
  size: string;
  price: number;
  shares: string;
  placed_at: number;
  pnl?: string;
  status: string;
};

// ---------- Queries ----------
function useKpis() {
  return useQuery({
    queryKey: ['kpis'],
    queryFn: () => invoke<DashboardKpis>('dashboard_kpis'),
    refetchInterval: 30_000,
  });
}

function useSignals() {
  return useQuery({
    queryKey: ['signals', 'active'],
    queryFn: () => invoke<Signal[]>('list_active_signals', { args: { minEdge: 0.05, limit: 20 } }),
    refetchInterval: 60_000,
  });
}

function useBets() {
  return useQuery({
    queryKey: ['bets', 'recent'],
    queryFn: () => invoke<Bet[]>('list_bets', { args: { limit: 50 } }),
    refetchInterval: 60_000,
  });
}

// ---------- Page ----------
export function Dashboard() {
  const kpis = useKpis();
  const signals = useSignals();
  const bets = useBets();

  const open = (bets.data ?? []).filter((b) => b.status === 'open').slice(0, 4);

  return (
    <div className="px-6 py-5 space-y-5">
      {/* Page header */}
      <div className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Good evening, dutong</h1>
          <p className="text-xs mt-0.5" style={{ color: 'var(--muted)' }}>
            {kpis.data?.active_signals ?? 0} active signals · {kpis.data?.open_positions ?? 0} open positions ·{' '}
            <span style={{ color: 'var(--fg-secondary)' }}>@quantumorca</span> filled 4 orders
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Segmented options={['24H', '7D', '30D', 'All']} defaultValue="24H" />
          <button className="btn">
            <Download className="w-3.5 h-3.5" /> Export
          </button>
        </div>
      </div>

      {/* KPIs */}
      <div className="grid grid-cols-4 gap-3">
        <Kpi
          icon={<Wallet className="w-3.5 h-3.5" />}
          label="Total Equity"
          value={kpis.data ? `$${fmtNum(kpis.data.total_equity_usdc)}` : '—'}
          delta={<span className="font-mono text-bull">+$341.20 (+2.73%)</span>}
          deltaHint="24H"
        />
        <Kpi
          icon={<Activity className="w-3.5 h-3.5" />}
          label="Open P&L"
          value={kpis.data ? `+$${fmtNum(kpis.data.open_pnl_usdc)}` : '—'}
          valueClass="text-bull"
          delta={<span>Across {kpis.data?.open_positions ?? 0} positions</span>}
        />
        <Kpi
          icon={<Target className="w-3.5 h-3.5" />}
          label="Win Rate (30D)"
          value={kpis.data ? `${(kpis.data.win_rate_30d * 100).toFixed(1)}%` : '—'}
          delta={<span className="font-mono">47W · 22L</span>}
        />
        <Kpi
          icon={<FlaskConical className="w-3.5 h-3.5" />}
          label="Brier Score"
          value={kpis.data ? kpis.data.brier_score.toFixed(3) : '—'}
          delta={
            <>
              <CheckCircle2 className="w-3 h-3 text-bull" />
              <span className="text-bull">well-calibrated</span>
            </>
          }
        />
      </div>

      {/* Charts row */}
      <div className="grid grid-cols-3 gap-3">
        <div className="card col-span-2 p-4">
          <ChartHeader title="Equity Curve" subtitle="Cumulative P&L · 47 settled positions" />
          <EquityCurve />
        </div>
        <div className="card p-4">
          <ChartHeader title="Calibration" subtitle="Predicted vs actual" />
          <Calibration />
        </div>
      </div>

      {/* Signals table */}
      <div className="card overflow-hidden">
        <div className="flex items-center justify-between px-4 py-3" style={{ borderBottom: '1px solid var(--border)' }}>
          <div>
            <div className="text-[13px] font-medium">Active Signals</div>
            <div className="text-[11px]" style={{ color: 'var(--muted)' }}>
              Edge ≥ ±5% · live updated
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button className="btn">
              <Filter className="w-3.5 h-3.5" /> Edge &gt; 5%
            </button>
            <button className="btn">
              <RefreshCw className="w-3.5 h-3.5" /> Refresh
            </button>
            <button className="btn btn-primary">
              <Plus className="w-3.5 h-3.5" /> New Signal
            </button>
          </div>
        </div>
        <table>
          <thead>
            <tr>
              <th>Market</th>
              <th>Our</th>
              <th style={{ textAlign: 'right' }}>Mkt</th>
              <th style={{ textAlign: 'right' }}>Edge</th>
              <th>Conf</th>
              <th style={{ textAlign: 'right' }}>Action</th>
            </tr>
          </thead>
          <tbody>
            {(signals.data ?? MOCK_SIGNALS).map((s) => (
              <tr key={s.id}>
                <td>
                  <div className="font-medium text-[13px]">{s.market_question ?? `Market #${s.market_id.slice(0, 6)}`}</div>
                  <div className="text-[11px]" style={{ color: 'var(--muted)' }}>
                    {categoryOf(s.market_id)} · closes in 2h 14m
                  </div>
                </td>
                <td className="font-mono font-medium">{pct(s.predicted_prob)}</td>
                <td className="font-mono text-right" style={{ color: 'var(--fg-secondary)' }}>
                  {pct(s.market_prob)}
                </td>
                <td className={cn('font-mono font-semibold text-right', s.edge >= 0 ? 'text-bull' : 'text-bear')}>
                  {s.edge >= 0 ? '+' : ''}
                  {(s.edge * 100).toFixed(1)}%
                </td>
                <td>
                  <div className="flex items-center gap-2">
                    <div className="progress w-16">
                      <div className="progress-fill" style={{ width: `${s.confidence * 100}%`, background: s.edge >= 0 ? 'var(--bull)' : 'var(--bear)' }} />
                    </div>
                    <span className="text-[11px] font-mono" style={{ color: 'var(--muted)' }}>
                      {(s.confidence * 100).toFixed(0)}%
                    </span>
                  </div>
                </td>
                <td style={{ textAlign: 'right' }}>
                  <button className="btn mr-1">
                    <ExternalLink className="w-3 h-3" /> Jump
                  </button>
                  <button className="btn btn-primary">
                    <Copy className="w-3 h-3" /> Copy
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Bottom row */}
      <div className="grid grid-cols-2 gap-3">
        <div className="card p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="text-[13px] font-medium">Open Positions</div>
            <a className="text-[11px] text-accent hover:underline cursor-pointer">view all →</a>
          </div>
          <ul className="space-y-2.5">
            {(open.length ? open : MOCK_OPEN).map((b) => (
              <li key={b.id} className="flex items-center gap-3 text-[12px]">
                <span
                  className="w-1.5 h-1.5 rounded-full"
                  style={{ background: (parseFloat(b.pnl ?? '0') >= 0) ? 'var(--bull)' : 'var(--bear)' }}
                />
                <div className="flex-1 min-w-0">
                  <div className="truncate font-medium">
                    {b.side} · {b.market_id.slice(0, 12)}
                  </div>
                  <div className="text-[10.5px] font-mono" style={{ color: 'var(--muted)' }}>
                    {b.size} @ {b.price.toFixed(2)}
                  </div>
                </div>
                <div className="text-right">
                  <div className={cn('font-mono font-medium', parseFloat(b.pnl ?? '0') >= 0 ? 'text-bull' : 'text-bear')}>
                    {parseFloat(b.pnl ?? '0') >= 0 ? '+' : ''}${b.pnl ?? '0.00'}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </div>

        <div className="card p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="text-[13px] font-medium">Recent Activity</div>
            <a className="text-[11px] text-accent hover:underline cursor-pointer">view log →</a>
          </div>
          <ol className="space-y-3 text-[12px] relative pl-3" style={{ borderLeft: '1px solid var(--border)' }}>
            <ActivityItem color="accent" time="2m ago" body={<>Placed <span className="font-medium">YES · LGD Map 2</span> via Jump-to-Polymarket</>} />
            <ActivityItem color="bull" time="14m ago" body={<>Settled <span className="font-medium">YES · G2 win</span> <span className="font-mono text-bull">+$8.40</span> (Brier 0.121)</>} />
            <ActivityItem color="warning" time="1h ago" body={<>Signal regenerated: <span className="font-medium">Fed rate cut</span> (conf 78%)</>} />
            <ActivityItem color="muted" time="3h ago" body={<>Sync — <span className="font-mono">4,128 ticks</span>, 12 orderbook snapshots</>} />
          </ol>
        </div>
      </div>
    </div>
  );
}

// ---------- helpers ----------
function fmtNum(s: string): string {
  const n = parseFloat(s);
  if (!Number.isFinite(n)) return s;
  return n.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
function pct(p: number) {
  return `${(p * 100).toFixed(0)}%`;
}
function categoryOf(_id: string): string {
  return 'CS2';
}

function Kpi({
  icon,
  label,
  value,
  valueClass,
  delta,
  deltaHint,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  valueClass?: string;
  delta: React.ReactNode;
  deltaHint?: string;
}) {
  return (
    <div className="kpi">
      <div className="flex items-center justify-between">
        <div className="kpi-label">{label}</div>
        <div style={{ color: 'var(--muted)' }}>{icon}</div>
      </div>
      <div className={cn('kpi-value font-mono', valueClass)}>{value}</div>
      <div className="kpi-delta">
        {delta}
        {deltaHint && <span style={{ color: 'var(--muted)' }}> · {deltaHint}</span>}
      </div>
    </div>
  );
}

function ChartHeader({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div className="flex items-center justify-between mb-3">
      <div>
        <div className="text-[13px] font-medium">{title}</div>
        <div className="text-[11px]" style={{ color: 'var(--muted)' }}>
          {subtitle}
        </div>
      </div>
      <div className="flex items-center gap-3 text-[11px]" style={{ color: 'var(--muted)' }}>
        <span className="flex items-center gap-1.5">
          <span className="w-2 h-2 rounded-full" style={{ background: 'var(--accent)' }} />
          Equity
        </span>
        <span className="flex items-center gap-1.5">
          <span className="w-2 h-2 rounded-full" style={{ background: 'var(--border-strong)' }} />
          Baseline
        </span>
      </div>
    </div>
  );
}

function Segmented({ options, defaultValue }: { options: string[]; defaultValue: string }) {
  return (
    <div className="seg">
      {options.map((o) => (
        <button key={o} className={o === defaultValue ? 'active' : ''}>
          {o}
        </button>
      ))}
    </div>
  );
}

function ActivityItem({ color, time, body }: { color: 'accent' | 'bull' | 'bear' | 'warning' | 'muted'; time: string; body: React.ReactNode }) {
  const dotColor =
    color === 'bull'
      ? 'var(--bull)'
      : color === 'bear'
      ? 'var(--bear)'
      : color === 'warning'
      ? 'var(--warning)'
      : color === 'muted'
      ? 'var(--border-strong)'
      : 'var(--accent)';
  return (
    <li>
      <span
        className="absolute -left-[5px] w-2.5 h-2.5 rounded-full"
        style={{ background: dotColor, border: '2px solid var(--surface)' }}
      />
      <div className="text-[10.5px] font-mono" style={{ color: 'var(--muted)' }}>
        {time}
      </div>
      <div>{body}</div>
    </li>
  );
}

// ---------- inline SVGs ----------
function EquityCurve() {
  return (
    <>
      <svg viewBox="0 0 600 180" className="w-full h-44">
        <g stroke="var(--border)" strokeWidth="1">
          <line x1="0" y1="40" x2="600" y2="40" />
          <line x1="0" y1="90" x2="600" y2="90" />
          <line x1="0" y1="140" x2="600" y2="140" />
        </g>
        <line x1="0" y1="140" x2="600" y2="140" stroke="var(--border-strong)" strokeDasharray="3 3" />
        <defs>
          <linearGradient id="eqFill" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.18" />
            <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <path
          d="M0,150 L50,142 L100,148 L150,128 L200,118 L250,122 L300,100 L350,86 L400,72 L450,68 L500,48 L550,38 L600,28 L600,180 L0,180 Z"
          fill="url(#eqFill)"
        />
        <path
          d="M0,150 L50,142 L100,148 L150,128 L200,118 L250,122 L300,100 L350,86 L400,72 L450,68 L500,48 L550,38 L600,28"
          fill="none"
          stroke="var(--accent)"
          strokeWidth="2"
          strokeLinejoin="round"
        />
        <circle cx="600" cy="28" r="3" fill="var(--accent)" />
      </svg>
      <div className="flex items-center justify-between text-[10px] mt-1.5 px-1 font-mono" style={{ color: 'var(--muted)' }}>
        <span>Oct 10</span>
        <span>Oct 17</span>
        <span>Oct 24</span>
        <span>Oct 31</span>
        <span>Nov 7</span>
      </div>
    </>
  );
}

function Calibration() {
  return (
    <>
      <svg viewBox="0 0 220 180" className="w-full h-44">
        <line x1="20" y1="160" x2="200" y2="20" stroke="var(--border-strong)" strokeDasharray="3 3" />
        <line x1="20" y1="160" x2="200" y2="160" stroke="var(--border)" />
        <line x1="20" y1="20" x2="20" y2="160" stroke="var(--border)" />
        <circle cx="30" cy="152" r="4" fill="var(--accent)" />
        <circle cx="60" cy="142" r="4" fill="var(--accent)" />
        <circle cx="90" cy="124" r="4" fill="var(--accent)" />
        <circle cx="120" cy="104" r="4" fill="var(--accent)" />
        <circle cx="150" cy="88" r="4" fill="var(--accent)" />
        <circle cx="180" cy="48" r="4" fill="var(--accent)" />
        <text x="20" y="176" fill="var(--muted)" fontSize="9" fontFamily="JetBrains Mono, monospace">
          0%
        </text>
        <text x="178" y="176" fill="var(--muted)" fontSize="9" fontFamily="JetBrains Mono, monospace">
          100%
        </text>
        <text x="2" y="164" fill="var(--muted)" fontSize="9" fontFamily="JetBrains Mono, monospace">
          0
        </text>
        <text x="2" y="24" fill="var(--muted)" fontSize="9" fontFamily="JetBrains Mono, monospace">
          1
        </text>
      </svg>
      <div className="text-[10px] text-center mt-1" style={{ color: 'var(--muted)' }}>
        closer to diagonal = better
      </div>
    </>
  );
}

// ---------- mock fallback for offline / first-run ----------
const MOCK_SIGNALS: Signal[] = [
  { id: 1, market_id: '0xabc1', predicted_prob: 0.71, market_prob: 0.58, edge: 0.13, confidence: 0.82, market_question: 'LGD vs Spirit — Map 2 winner' },
  { id: 2, market_id: '0xabc2', predicted_prob: 0.63, market_prob: 0.54, edge: 0.09, confidence: 0.71, market_question: 'Real Madrid vs Barcelona' },
  { id: 3, market_id: '0xabc3', predicted_prob: 0.22, market_prob: 0.35, edge: -0.13, confidence: 0.78, market_question: 'Fed rate cut by Dec 2025' },
  { id: 4, market_id: '0xabc4', predicted_prob: 0.55, market_prob: 0.49, edge: 0.06, confidence: 0.64, market_question: 'NaVi vs FaZe — Map 1 handicap' },
];

const MOCK_OPEN: Bet[] = [
  { id: 'b1', market_id: '0xabc1', side: 'YES', size: '120', price: 0.58, shares: '206.9', placed_at: Date.now(), pnl: '24.30', status: 'open' },
  { id: 'b2', market_id: '0xabc2', side: 'YES', size: '80', price: 0.54, shares: '148.1', placed_at: Date.now(), pnl: '12.50', status: 'open' },
  { id: 'b3', market_id: '0xabc3', side: 'NO', size: '50', price: 0.35, shares: '142.8', placed_at: Date.now(), pnl: '-8.10', status: 'open' },
  { id: 'b4', market_id: '0xabc4', side: 'YES', size: '200', price: 0.49, shares: '408.2', placed_at: Date.now(), pnl: '11.00', status: 'open' },
];