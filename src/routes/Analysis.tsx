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
    // v0.16a — analysisId 现在是字符串（UUID），遵循 LlmAnalysis DTO。
    // v0.15c 必须用 `as unknown as number` 强制转换；该变通已不再需要。
    analysisId: string;
    consensusSide: string | null;
    consensusProb: number | null;
    consensusConfidence: number | null;
    /** v0.16b — 完整的推荐列表（从 `LlmAnalysis.recommendations`
     * 字段解析）。recMut 用它来为 `llmGetRecommendation` 挑选 rec id。*/
    recommendations: LlmRecommendation[];
  } | null>(null);
  // v0.15c — 跟踪正在执行的分析的 UUID，以便 AnalyzeProgress 组件
  // 能订阅到正确的事件。我们无法从 IPC 返回值中获取 id（事件在
  // IPC 解析完成之前触发）。因此我们监听下一个 `llm_analyze:started`
  // 事件，并从其 payload 中捕获 id。监听器在挂载时设置一次，
  // 并将 id 保存在 ref 中。
  const [activeAnalysisId, setActiveAnalysisId] = useState<string | null>(null);
  const expectedAnalysisRef = useRef<boolean>(false);
  useEffect(() => {
    let cancelled = false;
    const unsubPromise = onAnalyzeStarted((e: AnalyzeStartedEvent) => {
      if (cancelled) return;
      // 仅捕获用户刚刚点击 Analyze 后的事件
      if (expectedAnalysisRef.current) {
        setActiveAnalysisId(e.analysis_id);
        expectedAnalysisRef.current = false;
      }
    });
    return () => {
      cancelled = true;
      unsubPromise.then((u) => u()).catch(() => { /* 忽略 */ });
    };
  }, []);

  const [chosen, setChosen] = useState<LlmRecommendation | null>(null);

  const signals = useQuery({
    queryKey: ['signals', 'active', { limit: 50 }],
    queryFn: () => listActiveSignals({ limit: 50 }),
  });

  const queryClient = useQueryClient();
  const analyzeMut = useMutation({
    // v0.15c — 从 mutation 函数返回分析 id，以便调用方（onClick 处理函数）
    // 在 onSuccess 触发之前就知道 id。这样我们就可以立即设置 activeAnalysisId
    // 并开始监听事件。
    mutationFn: async (m: string) => {
      const a = await llmAnalyze(m);
      return { analysis: a, marketId: m };
    },
    onSuccess: (r) => {
      setAnalyzeResult({
        // v0.16a — analysisId 是来自 Rust DTO 的 UUID 字符串。
        // 不再需要类型转换（类型现在匹配）。
        analysisId: r.analysis.id,
        consensusSide: r.analysis.consensus_side,
        consensusProb: r.analysis.consensus_predicted,
        consensusConfidence: r.analysis.consensus_conf,
        // v0.16b — 保留 recommendations，以便 recMut 挑选 rec id
        // （Rust 的 `llm_get_recommendation` 命令接受的是 rec id，
        // 而不是 analysis id）。
        recommendations: r.analysis.recommendations,
      });
      // mutation 已经返回；finished 事件在 IPC 返回前已触发。
      // 清除进行中的 ID，使进度网格停止订阅。
      setActiveAnalysisId(null);
      queryClient.invalidateQueries({ queryKey: ['llm-analyses'] });
    },
    onError: (e: Error) => {
      toast.error(t('analysis.toast.failed'), e.message);
      setActiveAnalysisId(null);
    },
  });

  const recMut = useMutation({
    // v0.16b — Rust 的 `llm_get_recommendation` 命令接受的是
    // recommendation id（自增 i64），而不是 analysis UUID。
    // v0.15c 的调用传入 analysis id，Rust 反序列化器无法将其
    // 解析为 i64 → 静默 IPC 失败。
    //
    // 我们按 parse_ok 优先、再按 confidence 降序选择首选推荐，
    // 以便模态框展示用户最有可能跟单的 LLM。
    mutationFn: () => {
      // v0.16b — Rust 命令接受 rec id，而不是 analysis id。
      // 我们从当前 analyzeResult 的 recommendations 中挑选首选 rec。
      // （下方 Button 的 `mutate(analyzeResult.analysisId)` 调用
      // 是 v0.15c 遗留的参数 —— recMut 现在忽略它，
      // 转而使用闭包捕获的 analyzeResult。）
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
    // v0.16b — Rust 的 `record_llm_decision` 接受 `RecordDecisionArgs`
    // 结构体。L1 封装（v0.16b）以类型化对象的相同形式接收。
    // v0.15c 的调用传入 `(analysisId: number, decision: string)`，
    // 与 Rust 的参数结构体完全不一致 → 静默 IPC 失败。
    mutationFn: (decision: 'follow_top' | 'manual_yes' | 'manual_no' | 'skip' | 're_analyze') =>
      recordLlmDecision({
        analysisId: analyzeResult!.analysisId,
        userDecision: decision,
        // userDecidedSide、followedLlmId、betId、contextSnapshot
        // 均为可选 —— Rust 结构体的 Option<T> 默认为 None。
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
      {/* 运行分析 */}
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
              // v0.15c — 设置一个标志，表示下一个 `started` 事件
              // 是「我们的」。上方的 useEffect 会从该事件中捕获
              // analysis_id 并向下传递给 AnalyzeProgress。
              expectedAnalysisRef.current = true;
              analyzeMut.mutate(marketId.trim());
            }}
          >
            {t('analysis.btn.analyze')}
          </Button>
        </div>
        {/* v0.15c — 按 provider 的进度网格。订阅后端发出的 4 个
            LLM analyze 事件。id 由上方的 onAnalyzeStarted 监听器设置，
            该监听器在用户点击 Analyze 后下一个 `started` 事件触发时执行。*/}
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
            {/* v0.16b — recMut 接受 analysis id 并在内部
                挑选首选 rec。Rust 接收的是 recommendation id（i64），
                而不是 analysis UUID。 */}
            <Button
              variant="secondary"
              size="sm"
              // v0.16b — recMut 不再以 analysis id 作为参数；
              // 它使用闭包捕获的 analyzeResult.recommendations
              // 并挑选首选 rec 发送到 Rust 端。
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

      {/* 活动信号 */}
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
                className="w-full text-left rounded-md border border-border bg-surface-2 p-2.5 hover:bg-surface-hover transition-colors duration-base ease-out-cubic"
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
            {/* v0.16a — 根据 LlmRecommendation DTO，这些字段可为空 */}
            <Field label={t('analysis.recommendation.side')} value={chosen.side ?? '—'} />
            <Field label={t('analysis.recommendation.predicted')} value={chosen.predicted_prob != null ? fmtPct(chosen.predicted_prob) : '—'} />
            <Field label={t('analysis.recommendation.confidence')} value={chosen.confidence != null ? fmtConfidence(chosen.confidence) : '—'} />
            <Field label={t('analysis.recommendation.cost')} value={chosen.cost_cents != null ? fmtCents(chosen.cost_cents) : '—'} />
            <Field label={t('analysis.recommendation.latency')} value={chosen.latency_ms != null ? fmtLatency(chosen.latency_ms) : '—'} />
            {/* v0.16a — 字段名为 `reasoning` 而非 `rationale`（与 Rust DTO 一致）*/}
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
      <div className="flex items-center gap-1.5 text-xs text-muted font-semibold uppercase tracking-caption-uppercase">
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
      <div className={'text-body-sm ' + (mono ? 'font-mono' : '')}>{value}</div>
    </div>
  );
}
