import { useState } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { Sparkles, Play, TrendingUp, TrendingDown, RefreshCw } from 'lucide-react';
import { llmAnalyze, llmGetRecommendation, recordLlmDecision, listActiveSignals } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { Skeleton } from '@/components/feedback/Skeleton';
import { EmptyState } from '@/components/feedback/EmptyState';
import { Modal } from '@/components/feedback/Modal';
import { toast } from '@/stores/toast-store';
import { fmtPct, fmtConfidence, fmtLatency, fmtCents, fmtRelativeTime } from '@/lib/format';
import { downloadCsv, toCsv } from '@/lib/csv';
import type { LlmRecommendation } from '@/types/llm';

export function Analysis() {
  const [marketId, setMarketId] = useState('');
  const [analyzeResult, setAnalyzeResult] = useState<{
    analysisId: number;
    consensusSide: string | null;
    consensusProb: number | null;
    consensusConfidence: number | null;
  } | null>(null);
  const [chosen, setChosen] = useState<LlmRecommendation | null>(null);

  const signals = useQuery({
    queryKey: ['signals', 'active', { limit: 50 }],
    queryFn: () => listActiveSignals({ limit: 50 }),
  });

  const queryClient = useQueryClient();
  const analyzeMut = useMutation({
    mutationFn: (m: string) => llmAnalyze(m),
    onSuccess: (a) => {
      setAnalyzeResult({
        analysisId: a.id,
        consensusSide: a.consensus_side,
        consensusProb: a.consensus_prob,
        consensusConfidence: a.consensus_confidence,
      });
      queryClient.invalidateQueries({ queryKey: ['llm-analyses'] });
    },
    onError: (e: Error) => toast.error('Analysis failed', e.message),
  });

  const recMut = useMutation({
    mutationFn: (analysisId: number) => llmGetRecommendation(analysisId),
    onSuccess: (r) => setChosen(r),
  });

  const decisionMut = useMutation({
    mutationFn: (decision: string) => recordLlmDecision(analyzeResult!.analysisId, decision),
    onSuccess: () => {
      toast.success('Decision recorded');
      queryClient.invalidateQueries({ queryKey: ['llm-stats'] });
    },
  });

  const exportCsv = () => {
    if (!signals.data) return;
    const csv = toCsv(
      signals.data.map((s) => ({
        market_id: s.market_id,
        question: s.market_question ?? '',
        model: s.model_version,
        edge: s.edge,
        confidence: s.confidence,
        computed_at: s.computed_at,
      })),
    );
    downloadCsv(`signals_${Date.now()}.csv`, csv);
  };

  return (
    <div className="space-y-4">
      {/* Run analysis */}
      <Card
        title="Run multi-LLM analysis"
        description="Fan out to enabled providers, build consensus, return per-LLM recommendations."
      >
        <div className="flex items-center gap-2">
          <Input
            placeholder="market id (paste from /markets)"
            value={marketId}
            onChange={(e) => setMarketId(e.target.value)}
            className="flex-1 font-mono"
          />
          <Button
            variant="primary"
            size="sm"
            iconLeft={<Play className="w-3 h-3" />}
            loading={analyzeMut.isPending}
            disabled={!marketId.trim()}
            onClick={() => analyzeMut.mutate(marketId.trim())}
          >
            Analyze
          </Button>
        </div>
        {analyzeResult && (
          <div className="mt-4 grid grid-cols-3 gap-3">
            <ResultCard
              label="Consensus side"
              value={analyzeResult.consensusSide ?? '—'}
              icon={
                analyzeResult.consensusSide === 'YES'
                  ? TrendingUp
                  : analyzeResult.consensusSide === 'NO'
                  ? TrendingDown
                  : Sparkles
              }
            />
            <ResultCard
              label="Consensus prob"
              value={analyzeResult.consensusProb != null ? fmtPct(analyzeResult.consensusProb) : '—'}
            />
            <ResultCard
              label="Confidence"
              value={analyzeResult.consensusConfidence != null ? fmtConfidence(analyzeResult.consensusConfidence) : '—'}
            />
          </div>
        )}
        {analyzeResult && (
          <div className="mt-3 flex items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={() => recMut.mutate(analyzeResult.analysisId)}
              loading={recMut.isPending}
            >
              Show top recommendation
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => decisionMut.mutate('follow_top')}
            >
              I follow top
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => decisionMut.mutate('skip')}
            >
              Skip
            </Button>
          </div>
        )}
      </Card>

      {/* Active signals */}
      <Card
        title="Active signals"
        description="Top predictions by |edge|. Pick one to analyze."
        action={
          <Button variant="ghost" size="sm" iconLeft={<RefreshCw className="w-3 h-3" />} onClick={() => signals.refetch()}>
            Refresh
          </Button>
        }
      >
        {signals.isLoading ? (
          <div className="space-y-2">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-12" />
            ))}
          </div>
        ) : !signals.data || signals.data.length === 0 ? (
          <EmptyState
            icon={<Sparkles className="w-5 h-5" />}
            title="No active signals"
            description="Recompute signals first, then pick one to analyze."
          />
        ) : (
          <div className="space-y-1.5">
            {signals.data.slice(0, 10).map((s) => (
              <button
                key={s.id}
                onClick={() => {
                  setMarketId(s.market_id);
                  setAnalyzeResult(null);
                }}
                className="w-full text-left rounded-md border border-border bg-surface-2 p-2.5 hover:bg-surface-hover transition-colors"
              >
                <div className="flex items-center gap-3">
                  <div className="flex-1 min-w-0">
                    <div className="text-[12px] text-fg line-clamp-1">
                      {s.market_question ?? s.market_id}
                    </div>
                    <div className="text-[10px] text-muted mt-0.5">
                      {s.model_version} · {fmtRelativeTime(s.computed_at)}
                    </div>
                  </div>
                  <Pill kind={s.edge > 0 ? 'bull' : 'bear'}>
                    {s.edge > 0 ? 'YES' : 'NO'}
                  </Pill>
                  <span
                    className={
                      'font-mono font-semibold text-[12px] w-16 text-right ' +
                      (s.edge > 0 ? 'text-bull' : 'text-bear')
                    }
                  >
                    {fmtPct(s.edge)}
                  </span>
                  <span className="font-mono text-[12px] text-muted w-14 text-right">
                    c {fmtConfidence(s.confidence)}
                  </span>
                </div>
              </button>
            ))}
            <div className="pt-2 flex justify-end">
              <Button variant="ghost" size="xs" onClick={exportCsv}>
                Export top {Math.min(10, signals.data.length)} as CSV
              </Button>
            </div>
          </div>
        )}
      </Card>

      {chosen && (
        <Modal
          open
          onClose={() => setChosen(null)}
          title="Top recommendation"
          size="md"
        >
          <div className="space-y-2">
            <Field label="Provider" value={chosen.provider_id} mono />
            <Field label="Side" value={chosen.side} />
            <Field label="Predicted probability" value={fmtPct(chosen.predicted_prob)} />
            <Field label="Confidence" value={fmtConfidence(chosen.confidence)} />
            <Field label="Cost" value={fmtCents(chosen.cost_cents)} />
            <Field label="Latency" value={fmtLatency(chosen.latency_ms)} />
            {chosen.rationale && (
              <div>
                <div className="text-[11px] text-muted mb-1">Rationale</div>
                <div className="text-[12px] text-fg bg-surface-2 rounded p-2 max-h-40 overflow-y-auto">
                  {chosen.rationale}
                </div>
              </div>
            )}
          </div>
        </Modal>
      )}
    </div>
  );
}

function ResultCard({
  label,
  value,
  icon: Icon,
}: {
  label: string;
  value: string;
  icon?: React.ComponentType<{ className?: string }>;
}) {
  return (
    <div className="rounded-md border border-border bg-surface-2 p-3">
      <div className="flex items-center gap-1.5 text-[10px] text-muted uppercase tracking-wide">
        {Icon && <Icon className="w-3 h-3" />}
        {label}
      </div>
      <div className="text-[16px] font-mono font-semibold text-fg mt-1">{value}</div>
    </div>
  );
}

function Field({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <div className="text-[11px] text-muted mb-0.5">{label}</div>
      <div className={'text-[13px] ' + (mono ? 'font-mono' : '')}>{value}</div>
    </div>
  );
}
