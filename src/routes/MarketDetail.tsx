import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, ExternalLink, Zap, Activity, Sparkles } from 'lucide-react';
import { listMarkets, listActiveSignals } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { SmartMoneyTab } from '@/components/football/SmartMoneyTab';
import { NewsTab } from '@/components/football/NewsTab';
import { ScoreMatrix } from '@/components/football/ScoreMatrix';
import { CrowdOpinionTab } from '@/components/football/CrowdOpinionTab';
import { ReversionGauge } from '@/components/football/ReversionGauge';
import { SpikeBadge } from '@/components/football/SpikeBadge';
import { UmaDisputeBadge } from '@/components/football/UmaDisputeBadge';
import { fmtDate, fmtUsdc, fmtEdge, fmtConfidence } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { toast } from '@/stores/toast-store';

/**
 * `/markets/:id` 路由 —— 单个 market 详情页。
 *
 * **数据流**：
 *   1. 从 URL 拿 `id`
 *   2. `listMarkets({ active_only: false, limit: 1000 })` 拉所有 market → 客户端 find
 *   3. `listActiveSignals({ limit: 50 })` 拉信号 → 客户端 filter by market_id
 *   4. 渲染：基本信息 / 订单簿（来自 `price_snapshots`）/ 信号列表 / 「Trade」按钮
 *
 * **为什么拉全部 market 而不是 by id**：Rust 端 `list_markets_by_id` IPC 还没暴露，
 * L1 走 `list_markets` + 客户端 find（market 数 ≤ 1000，单页查 O(n)）。
 *
 * **状态机**：`loading` / `not_found`（id 不存在）/ `success` / `error`。
 */
export function MarketDetail() {
  const { t } = useT();
  const { id } = useParams<{ id: string }>();
  const queryClient = useQueryClient();

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

  // v0.121 — MarketDetail 页面上的「Run analysis」按钮。
  // Analysis 页（`/analysis`）虽然可以通过 URL 访问，但侧边栏
  // 并没有链接到它，因此缺少按市场的入口。该 mutation 调用
  // `llm_analyze` IPC 并使用 football.v1.0 prompt（当
  // market.category == "football" 时由 Rust 端自动选择），
  // 成功时使 signals 查询失效，以便新预测显示在下方。
  //
  // 触发方式：用户在 MarketDetail 上点击「Run analysis」。
  // Toast 显示成功/失败（延迟、成本、错误原因）。
  const analyzeMut = useMutation({
    mutationFn: () =>
      invoke('llm_analyze', {
        args: {
          market_id: id!,
          prompt_version: 'football.v1.0',
          triggered_by: 'user:marketdetail',
          provider_ids: ['MiniMax', 'doubao'],
        },
      }),
    onSuccess: (r: any) => {
      const latency = r?.total_latency_ms ?? 0;
      const cost = r?.cost_cents ?? 0;
      const side = r?.consensus_side ?? '—';
      const prob = r?.consensus_predicted != null
        ? (r.consensus_predicted * 100).toFixed(1) + '%'
        : '—';
      toast.success(
        'Analysis complete',
        `side=${side} prob=${prob} · ${latency}ms · ${cost.toFixed(2)}¢`
      );
      queryClient.invalidateQueries({ queryKey: ['signals'] });
    },
    onError: (e: Error) => toast.error('Analysis failed', e.message),
  });

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
          <ArrowLeft className="w-3 h-3" /> {t('marketdetail.back_markets')}
        </Link>
        <EmptyState title={t('marketdetail.not_found')} description={t('marketdetail.not_found_desc', { id: id ?? '?' })} />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <Link
        to="/markets"
        className="text-muted hover:text-fg text-[12px] inline-flex items-center gap-1"
      >
        <ArrowLeft className="w-3 h-3" /> {t('marketdetail.back_markets')}
      </Link>

      <Card>
        <div className="space-y-3">
          <div className="flex items-start justify-between gap-3">
            <div className="space-y-1 flex-1">
              <div className="flex items-center gap-2">
                <Pill kind="muted">{market.category}</Pill>
                {market.resolved ? (
                  <Pill kind="muted">{market.outcome ?? t('marketdetail.resolved')}</Pill>
                ) : market.active ? (
                  <Pill kind="bull">{t('marketdetail.active')}</Pill>
                ) : (
                  <Pill kind="muted">{t('marketdetail.inactive')}</Pill>
                )}
                {id && <SpikeBadge marketId={id} />}
                {id && <UmaDisputeBadge marketId={id} />}
              </div>
              <h1 className="text-title-md font-semibold text-fg leading-snug">
                {market.question}
              </h1>
            </div>
            <a
              href={`https://polymarket.com/event/${market.slug}`}
              target="_blank"
              rel="noopener noreferrer"
            >
              <Button variant="secondary" size="sm" iconRight={<ExternalLink className="w-3 h-3" />}>
                {t('marketdetail.btn.open_polymarket')}
              </Button>
            </a>
            {/* v0.121 — 按市场的「Run analysis」按钮。调用
                llm_analyze IPC 并使用 football.v1.0 prompt，
                成功时刷新 signals 卡片。在 mutation 进行中
                时禁用。*/}
            <Button
              variant="primary"
              size="sm"
              iconLeft={<Sparkles className="w-3 h-3" />}
              loading={analyzeMut.isPending}
              onClick={() => analyzeMut.mutate()}
              data-testid="run-analysis-btn"
            >
              {analyzeMut.isPending ? 'Analyzing…' : 'Run analysis'}
            </Button>
          </div>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-3 pt-2">
            <Stat label={t('marketdetail.stat.liquidity')} value={`$${fmtUsdc(market.liquidity)}`} />
            <Stat label={t('marketdetail.stat.volume_24h')} value={`$${fmtUsdc(market.volume_24h)}`} />
            <Stat label={t('marketdetail.stat.closes')} value={fmtDate(market.end_date)} />
            <Stat label={t('marketdetail.stat.slug')} value={market.slug} mono />
          </div>
        </div>
      </Card>

      <Card
        title={t('marketdetail.signals.title')}
        description={t('marketdetail.signals.desc', { n: marketSignals.length })}
      >
        {marketSignals.length === 0 ? (
          <EmptyState
            icon={<Zap className="w-5 h-5" />}
            title={t('marketdetail.signals.empty')}
            description={t('marketdetail.signals.empty_desc')}
          />
        ) : (
          <div className="space-y-2">
            {marketSignals.map((s) => (
              <div
                key={s.id}
                className="rounded-md border border-border bg-surface-2 p-3 flex items-center gap-4"
              >
                <div className="flex-1 min-w-0">
                  <div className="text-[11px] text-muted">{t('marketdetail.signals.model', { version: s.model_version })}</div>
                  <div className="font-mono text-body-sm mt-0.5">
                    {t('marketdetail.signals.predicted', {
                      prob: (s.predicted_prob * 100).toFixed(1),
                      market: (s.market_prob * 100).toFixed(1),
                    })}
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
                  <div className="text-[10px] text-muted">{t('marketdetail.signals.conf', { value: fmtConfidence(s.confidence) })}</div>
                </div>
                <div className="text-right text-[10px] text-muted shrink-0">
                  <div>{t('marketdetail.signals.horizon', { hours: s.horizon_hours })}</div>
                  <div>{fmtDate(s.computed_at)}</div>
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card title={t('marketdetail.activity.title')} description={t('marketdetail.activity.desc')}>
        <div className="flex items-center gap-2 text-[12px] text-muted py-2">
          <Activity className="w-3.5 h-3.5" />
          <span>{t('marketdetail.activity.body')}</span>
        </div>
      </Card>

      {/* v0.126 — Smart Money Score（P0-1） */}
      {id && market.category === 'football' && (
        <SmartMoneyTab marketId={id} />
      )}

      {/* v0.126 — News & Catalysts（P1-1） */}
      {id && market.category === 'football' && (
        <NewsTab marketId={id} />
      )}

      {/* v0.126 — Poisson Score Matrix（P2-1） */}
      {id && market.category === 'football' && (
        <ScoreMatrix marketId={id} />
      )}

      {/* Phase 1.1 — Crowd Opinion（按资本加权） */}
      {id && market.category === 'football' && (
        <CrowdOpinionTab marketId={id} />
      )}

      {/* Phase 1.2 — Mean Reversion（均值回归） */}
      {id && market.category === 'football' && (
        <ReversionGauge marketId={id} />
      )}
    </div>
  );
}

function Stat({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase">{label}</div>
      <div className={'text-body-sm mt-0.5 ' + (mono ? 'font-mono' : 'text-fg')}>
        {value}
      </div>
    </div>
  );
}
