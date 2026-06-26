/**
 * BacktestReport —— v0.43d。
 *
 * 展示 `backtestModel` 调用结果的 modal:
 *   - Brier mean(头部核心指标)
 *   - Calibration 分桶(5 个柱,预测值 vs 实际值)
 *   - Top winners / top losers(各 3 个)
 *
 * 父组件(ModelLab)以预填的 model_version 打开此 modal。
 * 用户在 textarea 中输入/粘贴 JSON 样本列表,
 * 点击 "Run" 后报告渲染。
 *
 * 为什么要用 JSON textarea 而不是精致的表单?
 *   - 与传输格式完全一致(L1 端需要以某种方式构造
 *     `BacktestSample[]`,这里跳过了表单层)
 *   - 用户可以从任意来源粘贴(电子表格、其他工具、
 *     手写列表)
 *   - 未来 v0.43+ 可以增加 "Pull from resolved markets"
 *     按钮,从 markets DB 预填
 *
 * 解析是宽松的:无效样本由 sidecar 静默跳过,
 * 而非由 L1 处理。
 */
import { useState, useEffect } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { BarChart3, Trophy, Skull, Activity, Database } from 'lucide-react';
import { Modal } from '@/components/feedback/Modal';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { useT } from '@/lib/i18n';
import {
  backtestModel,
  listPromoteHistory,
  listResolvedMarketsForBacktest,
  type BacktestResult,
  type BacktestSample,
  type PromoteHistoryEntry,
  type ResolvedMarketSample,
} from '@/ipc';

interface BacktestReportProps {
  /** modal 是否打开。 */
  open: boolean;
  /** 用户关闭 modal 时的回调。 */
  onClose: () => void;
  /** v0.43c —— 待回测模型的 job_id
   * (从 PromoteHistory 单选)。组件从内存中的
   * history 查找 model_version。 */
  targetJobId: string | null;
}

const DEFAULT_SAMPLES_JSON = `[
  { "price": 0.3, "market_age_hours": 24.0, "outcome": 0.0, "label": "Will X happen?" },
  { "price": 0.7, "market_age_hours": 48.0, "outcome": 1.0, "label": "Will Y happen?" },
  { "price": 0.5, "market_age_hours": 12.0, "outcome": 0.5, "label": "Will Z happen?" }
]`;

function brierColor(brier: number): 'bull' | 'warn' | 'bear' {
  if (brier < 0.15) return 'bull';
  if (brier < 0.25) return 'warn';
  return 'bear';
}

export function BacktestReport({
  open,
  onClose,
  targetJobId,
}: BacktestReportProps) {
  const { t } = useT();
  const [targetEntry, setTargetEntry] = useState<PromoteHistoryEntry | null>(
    null,
  );
  const [samplesJson, setSamplesJson] = useState<string>(DEFAULT_SAMPLES_JSON);
  const [parseError, setParseError] = useState<string | null>(null);
  // v0.46 —— "Pull from resolved markets" 的数量上限。
  // L1 最多拉取该数量的 resolved market 并转换为
  // JSON 数组。默认 50(与 v0.46 IPC 默认一致)。
  const [pullLimit, setPullLimit] = useState<number>(50);

  // v0.43d —— 当 modal 带 target 打开时,拉取内存中
  // 的 history 来查找 model_version。比重新拉 archive
  // 代价更低。
  useEffect(() => {
    if (!open || !targetJobId) {
      setTargetEntry(null);
      return;
    }
    let cancelled = false;
    listPromoteHistory()
      .then((r) => {
        if (cancelled) return;
        const e = r.entries.find((x) => x.job_id === targetJobId);
        setTargetEntry(e ?? null);
      })
      .catch(() => {
        if (cancelled) return;
        setTargetEntry(null);
      });
    return () => {
      cancelled = true;
    };
  }, [open, targetJobId]);

  // v0.46 —— resolved markets 查询。供
  // "Pull from resolved markets" 按钮使用。
  // 仅在 modal 打开时启用,以避免每次页面
  // 访问都触发请求。
  const resolvedQuery = useQuery({
    queryKey: ['resolved-markets-for-backtest', pullLimit],
    queryFn: () => listResolvedMarketsForBacktest({ limit: pullLimit }),
    enabled: open,
    staleTime: 60_000,
  });

  // v0.46 —— "Pull from resolved markets" 按钮的处理函数。
  // 将查询结果转换为适合 textarea 的 JSON 数组。
  // v0.46 使用固定的 price=0.5 与 market_age_hours=24
  // (一种退化但一致的代理,代表"在 close 前 1 天预测")。
  // 如果用户手上有真实价格,可以在点击 Run 前
  // 编辑 textarea。
  const onPullResolved = () => {
    const samples = (resolvedQuery.data ?? []).map(
      (m: ResolvedMarketSample) => ({
        price: m.price,
        market_age_hours: m.market_age_hours,
        outcome: m.outcome === 'YES' ? 1.0 : 0.0,
        label: m.question,
      }),
    );
    if (samples.length === 0) {
      setParseError(t('backtest.no_resolved_markets'));
      return;
    }
    setSamplesJson(JSON.stringify(samples, null, 2));
    setParseError(null);
  };

  const mutation = useMutation({
    mutationFn: () => {
      if (!targetEntry) {
        throw new Error(t('backtest.no_target'));
      }
      let samples: BacktestSample[];
      try {
        const parsed = JSON.parse(samplesJson);
        if (!Array.isArray(parsed)) {
          throw new Error('samples must be a JSON array');
        }
        samples = parsed;
        setParseError(null);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setParseError(msg);
        throw new Error(t('backtest.invalid_json', { error: msg }));
      }
      return backtestModel({
        model_version: targetEntry.model_version,
        samples,
      });
    },
  });

  // 关闭时重置 mutation 状态
  useEffect(() => {
    if (!open) {
      mutation.reset();
      setParseError(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const result: BacktestResult | undefined = mutation.data;

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={
        <div className="flex items-center gap-2">
          <BarChart3 className="w-4 h-4" />
          <span>{t('backtest.title')}</span>
        </div>
      }
      size="lg"
    >
      <div className="space-y-3" data-testid="backtest-report">
        {/* Model 头部 —— 当前回测的是哪个 model */}
        {targetEntry ? (
          <div
            className="rounded-md border border-border bg-surface-2 p-2.5"
            data-testid="backtest-target"
            data-job-id={targetEntry.job_id}
          >
            <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase">
              {t('backtest.target_label')}
            </div>
            <div className="font-mono text-[12px] text-fg mt-0.5">
              {targetEntry.model_version}
            </div>
            <div className="text-[10px] text-muted mt-1">
              {t('backtest.target_train_brier', {
                brier:
                  targetEntry.best_brier !== null
                    ? targetEntry.best_brier.toFixed(4)
                    : '—',
              })}
            </div>
          </div>
        ) : (
          <Skeleton className="h-16" />
        )}

        {/* 样本输入 —— JSON textarea */}
        <div>
          <label
            className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase block mb-1"
            htmlFor="backtest-samples"
          >
            {t('backtest.samples_label')}
          </label>
          <textarea
            id="backtest-samples"
            data-testid="backtest-samples-input"
            value={samplesJson}
            onChange={(e) => setSamplesJson(e.target.value)}
            rows={8}
            className="w-full font-mono text-[11px] rounded border border-border bg-surface-1 p-2"
            spellCheck={false}
          />
          {parseError && (
            <div
              className="text-[10px] text-bear mt-1"
              data-testid="backtest-parse-error"
            >
              {t('backtest.invalid_json', { error: parseError })}
            </div>
          )}
        </div>

        {/* v0.46 —— 从 resolved markets 一键预填。
            一键从 markets DB(仅 resolved markets)预填。
            v0.46 使用退化代理(price=0.5、age=24h),
            已在 hint 中注明。用户可在点击 Run 前
            编辑 textarea。 */}
        <div className="flex items-center gap-2 flex-wrap">
          <Button
            data-testid="backtest-pull-resolved-btn"
            variant="secondary"
            size="sm"
            iconLeft={<Database className="w-3 h-3" />}
            onClick={onPullResolved}
            loading={resolvedQuery.isLoading}
            disabled={!targetEntry}
          >
            {t('backtest.pull_resolved')}
          </Button>
          <label
            className="text-[10px] text-muted"
            htmlFor="backtest-pull-limit"
          >
            {t('backtest.pull_limit')}
          </label>
          <input
            id="backtest-pull-limit"
            data-testid="backtest-pull-limit"
            type="number"
            min={1}
            max={500}
            value={pullLimit}
            onChange={(e) =>
              setPullLimit(Math.max(1, Math.min(500, Number(e.target.value) || 50)))
            }
            className="w-16 rounded border border-border bg-surface-1 px-2 py-1 text-[11px] font-mono"
          />
          {resolvedQuery.data && (
            <span className="text-[10px] text-muted">
              ({resolvedQuery.data.length} {t('backtest.resolved_available')})
            </span>
          )}
        </div>
        <p className="text-[10px] text-muted italic">
          {t('backtest.pull_hint')}
        </p>

        {/* Run 按钮 */}
        <div className="flex items-center gap-2">
          <Button
            data-testid="backtest-run-btn"
            size="sm"
            iconLeft={<Activity className="w-3 h-3" />}
            onClick={() => mutation.mutate()}
            loading={mutation.isPending}
            disabled={!targetEntry || mutation.isPending}
          >
            {t('backtest.run')}
          </Button>
          {result && result.ok && (
            <span className="text-[10px] text-muted">
              {t('backtest.sample_count', { n: result.sample_count })}
            </span>
          )}
        </div>

        {/* 错误 */}
        {mutation.isError && (
          <ErrorState
            title={t('backtest.error')}
            message={
              mutation.error instanceof Error
                ? mutation.error.message
                : String(mutation.error)
            }
          />
        )}

        {/* 结果 */}
        {result && result.ok && (
          <div
            className="space-y-3 border-t border-border pt-3"
            data-testid="backtest-result"
            data-brier={result.brier_mean ?? ''}
          >
            {/* 头部指标:Brier mean */}
            {result.brier_mean !== null && (
              <div className="flex items-baseline gap-2">
                <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase">
                  {t('backtest.brier_mean')}
                </div>
                <div
                  className={`font-mono text-[20px] text-${brierColor(result.brier_mean)}`}
                  data-testid="backtest-brier-mean"
                >
                  {result.brier_mean.toFixed(4)}
                </div>
              </div>
            )}

            {/* Calibration 柱 */}
            {result.calibration.length > 0 && (
              <div data-testid="backtest-calibration">
                <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase mb-1">
                  {t('backtest.calibration_title')}
                </div>
                <div className="space-y-1">
                  {result.calibration.map((b, i) => {
                    const hasData = b.count > 0;
                    const predicted = b.predicted_avg ?? 0;
                    const actual = b.actual_rate ?? 0;
                    return (
                      <div
                        key={i}
                        className="flex items-center gap-2 text-[10px]"
                        data-testid="backtest-cal-bucket"
                        data-bucket={b.bucket}
                        data-count={b.count}
                      >
                        <div className="w-14 text-muted font-mono">
                          {b.bucket}
                        </div>
                        <div className="flex-1 h-3 bg-surface-2 rounded relative overflow-hidden">
                          {/* Predicted(浅色,bull 色) */}
                          {hasData && (
                            <div
                              className="absolute top-0 left-0 h-full bg-bull/30"
                              style={{ width: `${predicted * 100}%` }}
                            />
                          )}
                          {/* Actual(深色,accent 色) */}
                          {hasData && (
                            <div
                              className="absolute top-0 left-0 h-1/2 bg-accent"
                              style={{ width: `${actual * 100}%` }}
                            />
                          )}
                        </div>
                        <div className="w-12 text-right text-fg font-mono">
                          {hasData ? `${b.count}` : '—'}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* 顶级赢家 */}
            {result.top_winners.length > 0 && (
              <div data-testid="backtest-top-winners">
                <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase mb-1 flex items-center gap-1">
                  <Trophy className="w-3 h-3 text-bull" />
                  {t('backtest.top_winners')}
                </div>
                <div className="space-y-1">
                  {result.top_winners.map((s, i) => (
                    <div
                      key={i}
                      className="flex items-center gap-2 text-[10px] border-l-2 border-bull pl-2"
                      data-testid="backtest-winner"
                      data-brier={s.brier}
                    >
                      <div className="font-mono text-bull w-12">
                        {s.brier.toFixed(3)}
                      </div>
                      <div className="flex-1 truncate text-fg">
                        {s.label || '—'}
                      </div>
                      <div className="text-muted">
                        p={s.predicted.toFixed(2)} → y={s.outcome.toFixed(2)}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* 顶级输家 */}
            {result.top_losers.length > 0 && (
              <div data-testid="backtest-top-losers">
                <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase mb-1 flex items-center gap-1">
                  <Skull className="w-3 h-3 text-bear" />
                  {t('backtest.top_losers')}
                </div>
                <div className="space-y-1">
                  {result.top_losers.map((s, i) => (
                    <div
                      key={i}
                      className="flex items-center gap-2 text-[10px] border-l-2 border-bear pl-2"
                      data-testid="backtest-loser"
                      data-brier={s.brier}
                    >
                      <div className="font-mono text-bear w-12">
                        {s.brier.toFixed(3)}
                      </div>
                      <div className="flex-1 truncate text-fg">
                        {s.label || '—'}
                      </div>
                      <div className="text-muted">
                        p={s.predicted.toFixed(2)} → y={s.outcome.toFixed(2)}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {/* 来自 sidecar 的 App 级错误(ok=false) */}
        {result && !result.ok && result.message && (
          <div
            className="text-[11px] text-bear bg-bear/10 border border-bear/30 rounded p-2"
            data-testid="backtest-app-error"
          >
            {result.message}
          </div>
        )}
      </div>
    </Modal>
  );
}
