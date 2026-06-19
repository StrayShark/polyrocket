// v0.78 — BankrollCard L4 component.
//
// Compact summary of bankroll state:
//   - Available (USDC) — total bankroll
//   - Reserved (USDC)   — never allocated (safety buffer)
//   - Allocated (USDC)  — sum of open bets
//   - Free (USDC)       — available - reserved - allocated
//
// Used in `/bankroll` route + Dashboard banner.
//
// @vitest-environment happy-dom

import { fmtUsdc } from '@/lib/format';
import { Wallet, Lock, TrendingUp, Unlock } from 'lucide-react';

export interface BankrollCardProps {
  /** Total available USDC. */
  availableUsdc: string;
  /** Reserved (never allocated) USDC. */
  reservedUsdc: string;
  /** Already-allocated (sum of open bets) USDC. */
  allocatedUsdc: string;
  /** Wallet label for context. */
  walletLabel?: string;
}

export function BankrollCard(props: BankrollCardProps) {
  const available = parseFloat(props.availableUsdc || '0') || 0;
  const reserved = parseFloat(props.reservedUsdc || '0') || 0;
  const allocated = parseFloat(props.allocatedUsdc || '0') || 0;
  const free = Math.max(0, available - reserved - allocated);

  return (
    <div
      className="grid grid-cols-2 md:grid-cols-4 gap-3"
      data-testid="bankroll-card"
    >
      <Tile
        icon={<Wallet className="w-4 h-4" />}
        label="Available"
        value={fmtUsdc(available)}
        tone="bull"
      />
      <Tile
        icon={<Lock className="w-4 h-4" />}
        label="Reserved"
        value={fmtUsdc(reserved)}
        tone="muted"
      />
      <Tile
        icon={<TrendingUp className="w-4 h-4" />}
        label="Allocated"
        value={fmtUsdc(allocated)}
        tone="accent"
      />
      <Tile
        icon={<Unlock className="w-4 h-4" />}
        label="Free"
        value={fmtUsdc(free)}
        tone={free > 0 ? 'bull' : 'bear'}
        highlight
      />
      {props.walletLabel && (
        <div className="col-span-2 md:col-span-4 text-[11px] text-muted">
          Wallet: {props.walletLabel}
        </div>
      )}
    </div>
  );
}

function Tile({
  icon,
  label,
  value,
  tone,
  highlight,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  tone: 'bull' | 'bear' | 'accent' | 'muted';
  highlight?: boolean;
}) {
  const toneClass = {
    bull: 'text-bull',
    bear: 'text-bear',
    accent: 'text-accent',
    muted: 'text-muted',
  }[tone];
  return (
    <div
      className={`p-3 rounded-md border border-border bg-surface ${
        highlight ? 'ring-1 ring-accent/30' : ''
      }`}
      data-testid={`bankroll-tile-${label.toLowerCase()}`}
    >
      <div className="flex items-center gap-1.5 text-[10px] text-muted uppercase tracking-wide">
        {icon}
        {label}
      </div>
      <div className={`mt-1 text-[15px] font-mono font-medium ${toneClass}`}>
        {value}
      </div>
    </div>
  );
}
