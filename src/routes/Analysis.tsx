import { useState, useRef, useEffect } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { Sparkles, Play, TrendingUp, TrendingDown, RefreshCw } from 'lucide-react';
import {
  llmAnalyze,
  llmGetRecommendation,
  recordLlmDecision,
  listActiveSignals,
  onAnalyzeStarted,
  type AnalyzeStartedEvent,
} from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { Skeleton } from '@/components/feedback/Skeleton';
import { EmptyState } from '@/components/feedback/EmptyState';
import { Modal } from '@/components/feedback/Modal';
import { AnalyzeProgress } from '@/components/feedback/AnalyzeProgress';
import { toast } from '@/stores/toast-store';
import { fmtPct, fmtConfidence, fmtLatency, fmtCents, fmtRelativeTime } from '@/lib/format';
import { downloadCsv, toCsv } from '@/lib/csv';
import { useT } from '@/lib/i18n';
import type { LlmRecommendation } from '@/types/llm';

export function Analysis() {
  const { t } = useT();
  const [marketId, setMarketId] = useState('');
  const [analyzeResult, setAnalyzeResult] = useState<{
    // v0.16a — analysisId is now a string (UUID) per the
    // LlmAnalysis DTO. v0.15c had to cast with `as unknown as
    // number`; that workaround is gone.
    analysisId: string;
    consensusSide: string | null;
    consensusProb: number | null;
    consensusConfidence: number | null;
    /** v0.16b — full recommendation list (parsed from the
     * `LlmAnalysis.recommendations` field). Used by recMut
     * to pick a rec id for `llmGetRecommendation`. */
    recommendations: LlmRecommendation[];
  } | null>(null);
  // v0.15c — track the UUID of the in-flight analyze so the
  // AnalyzeProgress component can subscribe to the right events.
  // We can't get the id from the IPC return value (events fire
  // BEFORE the IPC resolves). Instead, we listen for the next
  // `llm_analyze:started` event and capture the id from its
  // payload. The listener is set up once on mount and stashes
  // the id in a ref.
  const [activeAnalysisId, setActiveAnalysisId] = useState<string | null>(null);
  const expectedAnalysisRef = useRef<boolean>(false);
  useEffect(() => {
    let cancelled = false;
    const unsubPromise = onAnalyzeStarted((e: AnalyzeStartedEvent) => {
      if (cancelled) return;
      // Only capture if the user just clicked Analyze
      if (expectedAnalysisRef.current) {
        setActiveAnalysisId(e.analysis_id);
        expectedAnalysisRef.current = false;
      }
    });
    return () => {
      cancelled = true;
      unsubPromise.then((u) => u()).catch(() => { /* ignore */ });
    };
  }, []);

  const [chosen, setChosen] = useState<LlmRecommendation | null>(null);

  const signals = useQuery({
    queryKey: ['signals', 'active', { limit: 50 }],
    queryFn: () => listActiveSignals({ limit: 50 }),
  });

  const queryClient = useQueryClient();
  const analyzeMut = useMutation({
    // v0.15c — return the analysis id from the mutation function
    // so the caller (the onClick handler) knows the id before
    // onSuccess fires. This lets us set activeAnalysisId and
    // start listening to events immediately.
    mutationFn: async (m: string) => {
      const a = await llmAnalyze(m);
      return { analysis: a, marketId: m };
    },
    onSuccess: (r) => {
      setAnalyzeResult({
        // v0.16a — analysisId is the UUID string from the Rust
        // DTO. No cast needed (the type now matches).
        analysisId: r.analysis.id,
        consensusSide: r.analysis.consensus_side,
        consensusProb: r.analysis.consensus_predicted,
        consensusConfidence: r.analysis.consensus_conf,
        // v0.16b — keep the recommendations so recMut can pick
        // a rec id (the Rust `llm_get_recommendation` command
        // takes a rec id, NOT the analysis id).
        recommendations: r.analysis.recommendations,
      });
      // The mutation has already returned; the finished event
      // fired before the IPC returned. Clear the in-flight ID
      // so the progress grid stops subscribing.
      setActiveAnalysisId(null);
      queryClient.invalidateQueries({ queryKey: ['llm-analyses'] });
    },
    onError: (e: Error) => {
      toast.error(t('analysis.toast.failed'), e.message);
      setActiveAnalysisId(null);
    },
  });

  const recMut = useMutation({
    // v0.16b — the Rust `llm_get_recommendation` command takes
    // a recommendation id (auto-increment i64), NOT the analysis
    // UUID. v0.15c's call passed the analysis id, which the Rust
    // deserializer couldn't parse as i64 → silent IPC failure.
    //
    // We pick the top recommendation (by parse_ok, then by
    // confidence desc) so the modal shows the LLM the user is
    // most likely to follow.
    mutationFn: () => {
      // v0.16b — the Rust command takes a rec id, not the
      // analysis id. We pick the top rec from the current
      // analyzeResult's recommendations. (The Button's
      // `mutate(analyzeResult.analysisId)` call below is a
      // vestigial arg from v0.15c — recMut ignores it now
      // and uses the closed-over analyzeResult instead.)
      const recs = analyzeResult?.recommendations ?? [];
      const top = [...recs]
        .filter((r) => r.parse_ok)
        .sort((a, b) => (b.confidence ?? 0) - (a.confidence ?? 0))[0]
        ?? recs[0];
      if (!top) throw new Error('no recommendations to show');
      return llmGetRecommendation(top.id);
    },
    onSuccess: (r) => setChosen(r),
    onError: (e: Error) => toast.error(t('analysis.toast.failed'), e.message),
  });

  const decisionMut = useMutation({
    // v0.16b — Rust `record_llm_decision` takes a
    // `RecordDecisionArgs` struct. The L1 wrapper (v0.16b)
    // accepts the same shape as a typed object. v0.15c's call
    // passed `(analysisId: number, decision: string)` which
    // didn't match the Rust arg struct at all → silent IPC
    // failure.
    mutationFn: (decision: 'follow_top' | 'manual_yes' | 'manual_no' | 'skip' | 're_analyze') =>
      recordLlmDecision({
        analysisId: analyzeResult!.analysisId,
        userDecision: decision,
        // userDecidedSide, followedLlmId, betId, contextSnapshot
        // are optional — the Rust struct's Option<T> defaults
        // to None.
      }),
    onSuccess: () => {
      toast.success(t('analysis.toast.decision_recorded'));
      queryClient.invalidateQueries({ queryKey: ['llm-stats'] });
    },
    onError: (e: Error) => toast.error(t('analysis.toast.failed'), e.message),
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
        title={t('analysis.run.title')}
        description={t('analysis.run.desc')}
      >
        <div className="flex items-center gap-2">
          <Input
            placeholder={t('analysis.input.placeholder')}
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
            onClick={() => {
              // v0.15c — set a flag that the next `started` event
              // is "ours". The useEffect above will capture the
              // analysis_id from that event and pass it down to
              // AnalyzeProgress.
              expectedAnalysisRef.current = true;
              analyzeMut.mutate(marketId.trim());
            }}
          >
            {t('analysis.btn.analyze')}
          </Button>
        </div>
        {/* v0.15c — per-provider progress grid. Subscribes to the
            4 LLM analyze events emitted by the backend. The id
            is set by the onAnalyzeStarted listener (above) when
            the next `started` event fires after the user clicks
            Analyze. */}
        {activeAnalysisId && (
          <AnalyzeProgress
            key={activeAnalysisId}
            analysisId={activeAnalysisId}
            className="mt-3"
          />
        )}
        {analyzeResult && (
          <div className="mt-4 grid grid-cols-3 gap-3">
            <ResultCard
              label={t('analysis.kpi.side')}
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
              label={t('analysis.kpi.prob')}
              value={analyzeResult.consensusProb != null ? fmtPct(analyzeResult.consensusProb) : '—'}
            />
            <ResultCard
              label={t('analysis.kpi.confidence')}
              value={analyzeResult.consensusConfidence != null ? fmtConfidence(analyzeResult.consensusConfidence) : '—'}
            />
          </div>
        )}
        {analyzeResult && (
          <div className="mt-3 flex items-center gap-2">
            {/* v0.16b — recMut takes the analysis id and
                internally picks the top rec. Rust receives
                a recommendation id (i64), not the analysis
                UUID. */}
            <Button
              variant="secondary"
              size="sm"
              // v0.16b — recMut no longer takes the analysis
              // id as an arg; it uses the closed-over
              // analyzeResult.recommendations and picks the
              // top rec to send to the Rust side.
              onClick={() => recMut.mutate()}
              loading={recMut.isPending}
            >
              {t('analysis.btn.recommendation')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => decisionMut.mutate('follow_top')}
            >
              {t('analysis.btn.follow')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => decisionMut.mutate('skip')}
            >
              {t('analysis.btn.skip')}
            </Button>
          </div>
        )}
      </Card>

      {/* Active signals */}
      <Card
        title={t('analysis.signals.title')}
        description={t('analysis.signals.desc')}
        action={
          <Button variant="ghost" size="sm" iconLeft={<RefreshCw className="w-3 h-3" />} onClick={() => signals.refetch()}>
            {t('analysis.signals.refresh')}
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
            title={t('analysis.signals.empty')}
            description={t('analysis.signals.empty_desc')}
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
                {t('analysis.signals.export', { n: Math.min(10, signals.data.length) })}
              </Button>
            </div>
          </div>
        )}
      </Card>

      {chosen && (
        <Modal
          open
          onClose={() => setChosen(null)}
          title={t('analysis.recommendation.title')}
          size="md"
        >
          <div className="space-y-2">
            <Field label={t('analysis.recommendation.provider')} value={chosen.provider_id} mono />
            {/* v0.16a — fields are nullable per the LlmRecommendation DTO */}
            <Field label={t('analysis.recommendation.side')} value={chosen.side ?? '—'} />
            <Field label={t('analysis.recommendation.predicted')} value={chosen.predicted_prob != null ? fmtPct(chosen.predicted_prob) : '—'} />
            <Field label={t('analysis.recommendation.confidence')} value={chosen.confidence != null ? fmtConfidence(chosen.confidence) : '—'} />
            <Field label={t('analysis.recommendation.cost')} value={chosen.cost_cents != null ? fmtCents(chosen.cost_cents) : '—'} />
            <Field label={t('analysis.recommendation.latency')} value={chosen.latency_ms != null ? fmtLatency(chosen.latency_ms) : '—'} />
            {/* v0.16a — `reasoning` not `rationale` (matches Rust DTO) */}
            {chosen.reasoning && (
              <div>
                <div className="text-[11px] text-muted mb-1">{t('analysis.recommendation.rationale')}</div>
                <div className="text-[12px] text-fg bg-surface-2 rounded p-2 max-h-40 overflow-y-auto">
                  {chosen.reasoning}
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
