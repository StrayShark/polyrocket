import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Activity, Search, RefreshCw, Filter } from 'lucide-react';
import { listAuditLog } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Input } from '@/components/base/Input';
import { DataTable, type Column } from '@/components/data/DataTable';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { useDebounce } from '@/hooks/useDebounce';
import { fmtDateTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import type { AuditEntry } from '@/types/shared';

export function Audit() {
  const { t } = useT();
  const [search, setSearch] = useState('');
  const [actionFilter, setActionFilter] = useState<string | null>(null);
  const debouncedSearch = useDebounce(search, 200);

  const { data, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: ['audit-log'],
    queryFn: () => listAuditLog(500),
    staleTime: 30_000,
  });

  // Pull unique action prefixes for filter chips
  const actions = useMemo(() => {
    const set = new Set<string>();
    (data ?? []).forEach((e) => {
      const dot = e.action.indexOf('.');
      if (dot > 0) set.add(e.action.slice(0, dot));
    });
    return Array.from(set).sort();
  }, [data]);

  const filtered = useMemo(() => {
    if (!data) return [];
    const q = debouncedSearch.toLowerCase().trim();
    return data.filter((e) => {
      if (actionFilter && !e.action.startsWith(actionFilter + '.')) return false;
      if (q && !e.action.toLowerCase().includes(q) &&
          !(e.target ?? '').toLowerCase().includes(q) &&
          !(e.actor ?? '').toLowerCase().includes(q)) {
        return false;
      }
      return true;
    });
  }, [data, debouncedSearch, actionFilter]);

  const columns: Column<AuditEntry>[] = [
    {
      key: 'at',
      header: t('audit.col.when'),
      width: '160px',
      cell: (e) => <span className="font-mono text-[11px]">{fmtDateTime(e.at)}</span>,
      sortable: true,
      sortValue: (e) => e.at,
    },
    {
      key: 'actor',
      header: t('audit.col.actor'),
      width: '100px',
      cell: (e) => <Pill kind="muted">{e.actor}</Pill>,
      sortable: true,
      sortValue: (e) => e.actor,
    },
    {
      key: 'action',
      header: t('audit.col.action'),
      width: '180px',
      cell: (e) => <span className="font-mono text-[11px] text-fg">{e.action}</span>,
      sortable: true,
      sortValue: (e) => e.action,
    },
    {
      key: 'target',
      header: t('audit.col.target'),
      width: '180px',
      cell: (e) => e.target ? <code className="font-mono text-[11px] text-muted">{e.target}</code> : <span className="text-muted">—</span>,
    },
    {
      key: 'result',
      header: t('audit.col.result'),
      width: '90px',
      cell: (e) => (
        <Pill kind={e.result === 'ok' ? 'bull' : 'bear'}>
          {e.result}
        </Pill>
      ),
      sortable: true,
      sortValue: (e) => e.result,
    },
    {
      key: 'payload',
      header: t('audit.col.payload'),
      cell: (e) => e.payload ? (
        <pre className="font-mono text-[10px] text-muted whitespace-pre-wrap max-w-md truncate">
          {e.payload}
        </pre>
      ) : <span className="text-muted">—</span>,
    },
  ];

  return (
    <div className="space-y-4">
      <Card padding="sm">
        <div className="flex items-center gap-2 flex-wrap">
          <div className="relative flex-1 min-w-[200px]">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted" />
            <Input
              placeholder={t('audit.search.placeholder')}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-8"
            />
          </div>
          <div className="flex items-center gap-1 flex-wrap">
            <button
              onClick={() => setActionFilter(null)}
              className={
                'h-7 px-2.5 rounded text-[11px] font-medium border transition-colors ' +
                (actionFilter === null
                  ? 'bg-accent/15 text-accent border-accent/30'
                  : 'bg-surface-2 text-muted border-border hover:text-fg')
              }
            >
              {t('audit.filter.all')}
            </button>
            {actions.slice(0, 6).map((a) => (
              <button
                key={a}
                onClick={() => setActionFilter(a)}
                className={
                  'h-7 px-2.5 rounded text-[11px] font-medium border transition-colors ' +
                  (actionFilter === a
                    ? 'bg-accent/15 text-accent border-accent/30'
                    : 'bg-surface-2 text-muted border-border hover:text-fg')
                }
              >
                {a}.*
              </button>
            ))}
          </div>
          <div className="flex-1" />
          <button
            onClick={() => refetch()}
            disabled={isRefetching}
            className="h-7 px-2.5 rounded text-[11px] font-medium border bg-surface-2 text-muted border-border hover:text-fg inline-flex items-center gap-1"
          >
            <RefreshCw className={'w-3 h-3 ' + (isRefetching ? 'animate-spin' : '')} /> {t('audit.btn.refresh')}
          </button>
        </div>
      </Card>

      {error ? (
        <ErrorState message={String(error)} onRetry={() => refetch()} />
      ) : isLoading ? (
        <Card>
          <div className="space-y-2">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} className="h-9" />
            ))}
          </div>
        </Card>
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={<Activity className="w-5 h-5" />}
          title={t('audit.empty')}
          description={
            data && data.length === 0
              ? t('audit.empty.no_writes')
              : t('audit.empty.no_match')
          }
        />
      ) : (
        <DataTable rows={filtered} columns={columns} rowKey={(e) => String(e.id)} pageSize={30} />
      )}

      <div className="text-[10px] text-muted flex items-center gap-1.5 px-1">
        <Filter className="w-3 h-3" />
        {t('audit.footer', { shown: filtered.length, total: data?.length ?? 0 })}
      </div>
    </div>
  );
}
