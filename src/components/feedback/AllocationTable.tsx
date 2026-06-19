// v0.78 — AllocationTable L4 component.
//
// Renders an `AllocationResult.per_market` array as a table:
//   - market_id (truncated)
//   - side (Yes/No pill)
//   - size_usdc
//   - kelly_pct (with %)
//   - confidence
//   - expected_roi
//   - capped_reason (badge if any)
//
// @vitest-environment happy-dom

import { fmtUsdc, fmtPct, fmtConfidence } from '@/lib/format';
import { Pill } from '@/components/base/Pill';
import type { AllocationItem as Item } from '@/types/bankroll';

export function AllocationTable({ items }: { items: Item[] }) {
  if (items.length === 0) {
    return (
      <div
        className="text-center text-[12px] text-muted py-8"
        data-testid="allocation-table-empty"
      >
        No allocations — all signals filtered out (edge &lt; min or
        confidence &lt; threshold).
      </div>
    );
  }
  return (
    <table
      className="w-full text-[12px]"
      data-testid="allocation-table"
    >
      <thead>
        <tr className="text-left text-muted border-b border-border">
          <th className="py-1.5 font-medium">Market</th>
          <th className="py-1.5 font-medium">Side</th>
          <th className="py-1.5 font-medium text-right">Size</th>
          <th className="py-1.5 font-medium text-right">Kelly</th>
          <th className="py-1.5 font-medium text-right">Conf</th>
          <th className="py-1.5 font-medium text-right">E[ROI]</th>
          <th className="py-1.5 font-medium">Cap</th>
        </tr>
      </thead>
      <tbody>
        {items.map((item) => (
          <tr
            key={item.market_id}
            className="border-b border-border/50"
            data-testid={`allocation-row-${item.market_id}`}
          >
            <td className="py-1.5 font-mono text-fg">
              {item.market_id.slice(0, 12)}
              {item.market_id.length > 12 ? '…' : ''}
            </td>
            <td className="py-1.5">
              <Pill kind={item.side === 'Yes' ? 'bull' : 'bear'}>
                {item.side}
              </Pill>
            </td>
            <td
              className="py-1.5 text-right font-mono"
              data-testid={`alloc-size-${item.market_id}`}
            >
              {fmtUsdc(parseFloat(item.size_usdc) || 0)}
            </td>
            <td
              className="py-1.5 text-right font-mono"
              data-testid={`alloc-kelly-${item.market_id}`}
            >
              {fmtPct(item.kelly_pct)}
            </td>
            <td className="py-1.5 text-right font-mono">
              {fmtConfidence(item.confidence)}
            </td>
            <td
              className={`py-1.5 text-right font-mono ${
                item.expected_roi > 0 ? 'text-bull' : 'text-bear'
              }`}
            >
              {fmtPct(item.expected_roi)}
            </td>
            <td className="py-1.5">
              {item.capped_reason && (
                <Pill kind="muted">
                  {capLabel(item.capped_reason)}
                </Pill>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function capLabel(reason: 'PerSignalCap' | 'Liquidity' | 'TotalExposure'): string {
  switch (reason) {
    case 'PerSignalCap': return 'per-cap';
    case 'Liquidity':    return 'liq';
    case 'TotalExposure':return 'total';
  }
}
