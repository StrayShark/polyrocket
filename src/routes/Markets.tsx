import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { RefreshCw, Search, ExternalLink } from 'lucide-react';
import { Link } from 'react-router-dom';
import { listMarkets, syncMarkets } from '@/ipc';
import { DataTable, type Column } from '@/components/data/DataTable';
import { Input } from '@/components/base/Input';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Card } from '@/components/base/Card';
import { useT } from '@/lib/i18n';
import { EmptyState } from '@/components/feedback/EmptyState';
import { ErrorState } from '@/components/feedback/ErrorState';
import { Skeleton } from '@/components/feedback/Skeleton';
import { toast } from '@/stores/toast-store';
import { useDebounce } from '@/hooks/useDebounce';
import { fmtUsdc, fmtDate, fmtRelativeTime } from '@/lib/format';
import type { Market } from '@/types/market';

// v0.119 — 仅 football 产品（依据 docs/polyrocket-football-prd.md）。
// 后端仍同步所有 Polymarket 分类以备未来扩展，但 UI 仅暴露 football
// market。Category filter 简化为 ['all', 'football']，默认值为 'football'，
// 让用户立即进入产品主界面。保留 'all' 用于调试或想查看数据库中
// 其他分类的高级用户。
const CATEGORIES = ['all', 'football'] as const;
type Category = (typeof CATEGORIES)[number];

/**
 * `/markets` 路由 —— 浏览 / 搜索 / 筛选 Polymarket market (football-only)。
 *
 * **v0.119 — football pivot**：polyrocket 是 football prediction
 * terminal（per docs/polyrocket-football-prd.md）。UI 只暴露 football
 * 分类。后端 `markets` 表仍有 cs2/politics/crypto 等数据（保留为
 * future 扩展），但 sidebar / 页面 filter / 默认值都锁在 football。
 *
 * **数据流**：
 *   1. mount 时 `listMarkets({ active_only, limit: 500 })` 拉第一批
 *   2. 客户端按 `category=football` + `search` 过滤（`useDebounce(200ms)`）
 *   3. 表格分页（`DataTable` 内部处理）
 *   4. 「Sync」按钮调 `syncMarkets` mutation 触发后端重新从 Polymarket 拉
 *
 * **状态机**：`loading`（Skeleton）→ `success`（DataTable） / `error`（ErrorState）。
 *
 * **性能**：`staleTime: 60_000` 让 1 分钟内不重拉；`active_only` 是 query key
 * 的一部分，filter 切到 inactive 自动 refetch。
 */
export function Markets() {
  const { t } = useT();
  // v0.119 — 默认 football（之前为 'all'），让产品主界面
  // 立即落在 football market 上。
  const [category, setCategory] = useState<Category>('football');
  const [activeOnly, setActiveOnly] = useState(true);
  const [search, setSearch] = useState('');
  const debounced = useDebounce(search, 200);

  const { data, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: ['markets', { activeOnly }],
    queryFn: () => listMarkets({ active_only: activeOnly, limit: 500 }),
    staleTime: 60_000,
  });

  const queryClient = useQueryClient();
  const syncMut = useMutation({
    mutationFn: () => syncMarkets(),
    onSuccess: (n) => {
      toast.success(`Synced ${n} markets`);
      queryClient.invalidateQueries({ queryKey: ['markets'] });
    },
    // v0.124 — Tauri v2 的 `invoke` 以纯字符串 reject，
    // 而不是 Error 实例，因此 `e.message` 为 undefined。
    // 强制转为 String，使 toast 文案显示真实失败原因
    // （网络错误、HTTP 500、JSON 解析失败等）。
    onError: (e: unknown) => {
      // v0.124 — Tauri v2 包装 IPC 拒绝值的方式不一致：
      //   - 有时 reject 值是纯字符串
      //     （AppError 的 `serialize_str(&to_string())`）
      //   - 有时是带 .message 的 Error
      //   - 有时是 `{ message: string, code?: string }`
      // 依次尝试，最后回退到 String() 强制转换。
      let msg: string;
      if (typeof e === 'string') {
        msg = e;
      } else if (e instanceof Error) {
        msg = e.message || e.toString();
      } else if (e && typeof e === 'object' && 'message' in e && typeof (e as any).message === 'string') {
        msg = (e as any).message;
      } else {
        try {
          msg = JSON.stringify(e);
        } catch {
          msg = String(e);
        }
      }
      // 去掉 AppError 在 to_string 中添加的 "Internal: " 前缀
      const cleaned = msg.replace(/^Internal:\s*/i, '').trim();
      toast.error('Sync failed', cleaned || msg);
    },
  });

  const filtered = useMemo(() => {
    if (!data) return [];
    const cat = category === 'all' ? null : category;
    const q = debounced.toLowerCase().trim();
    return data.filter((m) => {
      if (cat && m.category !== cat) return false;
      if (q && !m.question.toLowerCase().includes(q) && !m.slug.toLowerCase().includes(q)) {
        return false;
      }
      return true;
    });
  }, [data, category, debounced]);

  const columns: Column<Market>[] = [
    {
      key: 'question',
      header: 'Question',
      cell: (m) => (
        <Link
          to={`/markets/${m.id}`}
          className="text-fg hover:text-accent line-clamp-2 max-w-[480px]"
        >
          {m.question}
        </Link>
      ),
      sortable: true,
      sortValue: (m) => m.question,
    },
    {
      key: 'category',
      header: 'Category',
      cell: (m) => <Pill kind="muted">{m.category}</Pill>,
      width: '110px',
      sortable: true,
      sortValue: (m) => m.category,
    },
    {
      key: 'liquidity',
      header: 'Liquidity',
      align: 'right',
      cell: (m) => (
        <span className="font-mono">${fmtUsdc(m.liquidity)}</span>
      ),
      width: '110px',
      sortable: true,
      sortValue: (m) => Number(m.liquidity ?? 0),
    },
    {
      key: 'volume_24h',
      header: '24h Vol',
      align: 'right',
      cell: (m) => (
        <span className="font-mono">${fmtUsdc(m.volume_24h)}</span>
      ),
      width: '100px',
      sortable: true,
      sortValue: (m) => Number(m.volume_24h ?? 0),
    },
    {
      key: 'end_date',
      header: 'Closes',
      cell: (m) => (
        <span title={fmtDate(m.end_date)}>{fmtRelativeTime(m.end_date)}</span>
      ),
      width: '120px',
      sortable: true,
      sortValue: (m) => m.end_date,
    },
    {
      key: 'status',
      header: 'Status',
      cell: (m) => (
        m.resolved ? (
          <Pill kind="muted">{m.outcome ?? 'resolved'}</Pill>
        ) : m.active ? (
          <Pill kind="bull">active</Pill>
        ) : (
          <Pill kind="muted">inactive</Pill>
        )
      ),
      width: '90px',
    },
    {
      key: 'actions',
      header: '',
      align: 'right',
      cell: (m) => (
        <a
          href={`https://polymarket.com/event/${m.slug}`}
          target="_blank"
          rel="noopener noreferrer"
          className="text-muted hover:text-accent inline-flex items-center"
          title="Open on Polymarket"
        >
          <ExternalLink className="w-3.5 h-3.5" />
        </a>
      ),
      width: '40px',
    },
  ];

  return (
    <div className="space-y-4">
      {/* 工具栏 */}
      <Card padding="sm">
        <div className="flex items-center gap-2 flex-wrap">
          <div className="relative flex-1 min-w-[200px]">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted" />
            <Input
              placeholder="Search question or slug…"  // v0.13a — 保留默认值；未来使用 `t('common.search')`
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-8"
            />
          </div>
          <div className="flex items-center gap-1">
            {CATEGORIES.map((c) => (
              <button
                key={c}
                onClick={() => setCategory(c)}
                className={
                  'h-7 px-2.5 rounded text-[11px] font-medium border transition-colors duration-base ease-out-cubic ' +
                  (category === c
                    ? 'bg-accent/15 text-accent border-accent/30'
                    : 'bg-surface-2 text-muted border-border hover:text-fg')
                }
              >
                {c}
              </button>
            ))}
          </div>
          <label className="flex items-center gap-1.5 text-[11px] text-muted">
            <input
              type="checkbox"
              checked={activeOnly}
              onChange={(e) => setActiveOnly(e.target.checked)}
              className="accent-accent"
            />
            active only
          </label>
          <div className="flex-1" />
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<RefreshCw className={'w-3 h-3 ' + (isRefetching ? 'animate-spin' : '')} />}
            onClick={() => refetch()}
            disabled={isRefetching}
          >
            Refresh
          </Button>
          <Button
            variant="primary"
            size="sm"
            loading={syncMut.isPending}
            onClick={() => syncMut.mutate()}
          >
            {t('markets.sync')}
          </Button>
        </div>
      </Card>

      {/* 表格 */}
      {error ? (
        <ErrorState message={String(error)} onRetry={() => refetch()} />
      ) : isLoading ? (
        <Card>
          <div className="space-y-2">
            {Array.from({ length: 8 }).map((_, i) => (
              <Skeleton key={i} className="h-7" />
            ))}
          </div>
        </Card>
      ) : filtered.length === 0 ? (
        <EmptyState
          title={t('status.empty')}
          description={
            search
              ? `No markets matching "${search}". Try a different query.`
              : t('markets.empty')
          }
        />
      ) : (
        <DataTable
          rows={filtered}
          columns={columns}
          rowKey={(m) => m.id}
          pageSize={25}
        />
      )}
    </div>
  );
}
