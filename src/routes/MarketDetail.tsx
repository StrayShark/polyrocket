import { useQuery } from '@tanstack/react-query';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, ExternalLink, Zap, Activity } from 'lucide-react';
import { listMarkets, listActiveSignals } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { fmtDate, fmtUsdc, fmtEdge, fmtConfidence } from '@/lib/format';

export function MarketDetail() {
  const { id } = useParams<{ id: string }>();

  const { data: markets, isLoading, error } = useQuery({
    queryKey: ['markets', { activeOnly: false }],
    queryFn: () => listMarkets({ active_only: false, limit: 1000 }),
    enabled: !!id,
  });

  const market = markets?.find((m) => m.id === id);

  const { data: signals } = useQuery({
    queryKey: ['signals', 'by-market', id],
    queryFn: () => listActiveSignals({ limit: 50 }),
    enabled: !!id,
  });

  const marketSignals = signals?.filter((s) => s.market_id === id) ?? [];

  if (error) return <ErrorState message={String(error)} />;
  if (isLoading) {
    return (
      <div className="space-y-3">
        <Skeleton className="h-8 w-1/2" />
        <Skeleton className="h-32" />
        <Skeleton className="h-48" />
      </div>
    );
  }
  if (!market) {
    return (
      <div className="space-y-3">
        <Link to="/markets" className="text-accent text-[12px] inline-flex items-center gap-1">
          <ArrowLeft className="w-3 h-3" /> Back to Markets
        </Link>
        <EmptyState title="Market not found" description={`No market with id ${id}`} />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <Link
        to="/markets"
        className="text-muted hover:text-fg text-[12px] inline-flex items-center gap-1"
      >
        <ArrowLeft className="w-3 h-3" /> Markets
      </Link>

      <Card>
        <div className="space-y-3">
          <div className="flex items-start justify-between gap-3">
            <div className="space-y-1 flex-1">
              <div className="flex items-center gap-2">
                <Pill kind="muted">{market.category}</Pill>
                {market.resolved ? (
                  <Pill kind="muted">{market.outcome ?? 'resolved'}</Pill>
                ) : market.active ? (
                  <Pill kind="bull">active</Pill>
                ) : (
                  <Pill kind="muted">inactive</Pill>
                )}
              </div>
              <h1 className="text-[18px] font-semibold text-fg leading-snug">
                {market.question}
              </h1>
            </div>
            <a
              href={`https://polymarket.com/event/${market.slug}`}
              target="_blank"
              rel="noopener noreferrer"
            >
              <Button variant="secondary" size="sm" iconRight={<ExternalLink className="w-3 h-3" />}>
                Open on Polymarket
              </Button>
            </a>
          </div>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-3 pt-2">
            <Stat label="Liquidity" value={`$${fmtUsdc(market.liquidity)}`} />
            <Stat label="24h Volume" value={`$${fmtUsdc(market.volume_24h)}`} />
            <Stat label="Closes" value={fmtDate(market.end_date)} />
            <Stat label="Slug" value={market.slug} mono />
          </div>
        </div>
      </Card>

      <Card
        title="Active signals"
        description={`${marketSignals.length} signal(s) for this market`}
      >
        {marketSignals.length === 0 ? (
          <EmptyState
            icon={<Zap className="w-5 h-5" />}
            title="No active signals"
            description="Run recompute_signals to generate a prediction for this market."
          />
        ) : (
          <div className="space-y-2">
            {marketSignals.map((s) => (
              <div
                key={s.id}
                className="rounded-md border border-border bg-surface-2 p-3 flex items-center gap-4"
              >
                <div className="flex-1 min-w-0">
                  <div className="text-[11px] text-muted">model {s.model_version}</div>
                  <div className="font-mono text-[13px] mt-0.5">
                    predicted {(s.predicted_prob * 100).toFixed(1)}% · market{' '}
                    {(s.market_prob * 100).toFixed(1)}%
                  </div>
                </div>
                <div className="text-right">
                  <div
                    className={
                      'text-[14px] font-mono font-semibold ' +
                      (s.edge > 0 ? 'text-bull' : 'text-bear')
                    }
                  >
                    {fmtEdge(s.edge)}
                  </div>
                  <div className="text-[10px] text-muted">conf {fmtConfidence(s.confidence)}</div>
                </div>
                <div className="text-right text-[10px] text-muted shrink-0">
                  <div>{s.horizon_hours}h horizon</div>
                  <div>{fmtDate(s.computed_at)}</div>
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card title="Activity" description="Coming soon — M5 copy events + M3 bet history">
        <div className="flex items-center gap-2 text-[12px] text-muted py-2">
          <Activity className="w-3.5 h-3.5" />
          <span>Activity timeline will be wired in M3 + M5.</span>
        </div>
      </Card>
    </div>
  );
}

function Stat({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <div className="text-[10px] text-muted uppercase tracking-wide">{label}</div>
      <div className={'text-[13px] mt-0.5 ' + (mono ? 'font-mono' : 'text-fg')}>
        {value}
      </div>
    </div>
  );
}
