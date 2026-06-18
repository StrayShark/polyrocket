import { useMemo, useState, type ReactNode } from 'react';
import { ChevronDown, ChevronUp, ChevronsUpDown } from 'lucide-react';
import { cn } from '@/lib/cn';

/** DataTable 列定义。**泛型 `<T>`** 是 row 类型（route 传 `Market` / `Bet` / `Signal` 等）。
 *
 * **字段**：
 *   - `key` — unique column key（用于 sort）
 *   - `header` — 表头 ReactNode（字符串 / icon / i18n key 都行）
 *   - `cell` — 渲染每行 cell（`row => ReactNode`）
 *   - `align` — `'left'` / `'right'` / `'center'`
 *   - `width` — CSS 宽度（`'100px'` / `'30%'` 等）
 *   - `sortable` — 是否可点表头排序
 *   - `sortValue` — 自定义 sort key（默认用 `cell` 字符串 fallback）
 */
export interface Column<T> {
  key: string;
  header: ReactNode;
  cell: (row: T) => ReactNode;
  align?: 'left' | 'right' | 'center';
  width?: string;
  sortable?: boolean;
  sortValue?: (row: T) => string | number | null;
}

/** `DataTable` props。**泛型 `<T>`** = row 类型。
 *
 * **必填**：`rows` / `columns` / `rowKey`。
 * **可选**：
 *   - `empty` — rows 为空时渲染（默认 EmptyState）
 *   - `loading` — Skeleton 模式
 *   - `pageSize` — 0 = 不分页（默认 0）。L1 路由如果 list 长可设 50。
 *   - `onRowClick` — 点击行回调（详情页用）
 *   - `className` — 外层 div className
 */
export interface DataTableProps<T> {
  rows: T[];
  columns: Column<T>[];
  rowKey: (row: T) => string;
  empty?: ReactNode;
  loading?: boolean;
  /** Default page size if pagination is on. 0 = no pagination. */
  pageSize?: number;
  onRowClick?: (row: T) => void;
  className?: string;
}

/**
 * `DataTable<T>` —— 通用表格 component（L1 表格的 baseline 实现）。
 *
 * **特性**：
 *   - 点击表头排序（asc / desc / 三态 cycle）
 *   - 客户端分页（`pageSize > 0` 时启用）
 *   - loading 态渲染 Skeleton 行
 *   - 行点击回调（`onRowClick`）
 *   - 三主题：颜色 / spacing / 字体走 `data-theme` CSS variable
 *
 * **何时不用 DataTable**：
 *   - 复杂 cell 交互（dropdown / popover）—— 写自定义 table
 *   - 树形结构（groups / sub-rows）—— 写 tree
 *   - 大数据虚拟化（> 10k 行）—— 加 `react-window` 再来
 *
 * @example
 *   const cols: Column<Market>[] = [
 *     { key: 'q', header: 'Question', cell: (m) => m.question, sortable: true,
 *       sortValue: (m) => m.question },
 *   ];
 *   <DataTable rows={markets} columns={cols} rowKey={(m) => m.id} pageSize={50} />
 */
export function DataTable<T>({
  rows,
  columns,
  rowKey,
  empty,
  loading,
  pageSize = 0,
  onRowClick,
  className,
}: DataTableProps<T>) {
  const [sort, setSort] = useState<{ key: string; dir: 'asc' | 'desc' } | null>(null);
  const [page, setPage] = useState(0);

  const sorted = useMemo(() => {
    if (!sort) return rows;
    const col = columns.find((c) => c.key === sort.key);
    if (!col?.sortValue) return rows;
    const sv = col.sortValue;
    const out = [...rows].sort((a, b) => {
      const va = sv(a);
      const vb = sv(b);
      if (va === null && vb === null) return 0;
      if (va === null) return 1;
      if (vb === null) return -1;
      if (va < vb) return sort.dir === 'asc' ? -1 : 1;
      if (va > vb) return sort.dir === 'asc' ? 1 : -1;
      return 0;
    });
    return out;
  }, [rows, sort, columns]);

  const paged = useMemo(() => {
    if (pageSize <= 0) return sorted;
    const start = page * pageSize;
    return sorted.slice(start, start + pageSize);
  }, [sorted, page, pageSize]);

  const totalPages = pageSize > 0 ? Math.ceil(sorted.length / pageSize) : 1;

  return (
    <div className={cn('rounded-lg border bg-surface border-border overflow-hidden', className)}>
      <div className="overflow-x-auto">
        <table className="w-full text-[12px]">
          <thead>
            <tr className="border-b border-border bg-surface-2">
              {columns.map((c) => {
                const isSorted = sort?.key === c.key;
                return (
                  <th
                    key={c.key}
                    style={c.width ? { width: c.width } : undefined}
                    className={cn(
                      'h-8 px-3 font-medium text-muted',
                      c.align === 'right' && 'text-right',
                      c.align === 'center' && 'text-center',
                      (!c.align || c.align === 'left') && 'text-left',
                      c.sortable && 'cursor-pointer select-none hover:text-fg',
                    )}
                    onClick={() => {
                      if (!c.sortable) return;
                      setSort((s) => {
                        if (s?.key !== c.key) return { key: c.key, dir: 'desc' };
                        if (s.dir === 'desc') return { key: c.key, dir: 'asc' };
                        return null;
                      });
                    }}
                  >
                    <span className="inline-flex items-center gap-1">
                      {c.header}
                      {c.sortable &&
                        (isSorted ? (
                          sort?.dir === 'asc' ? (
                            <ChevronUp className="w-3 h-3" />
                          ) : (
                            <ChevronDown className="w-3 h-3" />
                          )
                        ) : (
                          <ChevronsUpDown className="w-3 h-3 opacity-30" />
                        ))}
                    </span>
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {loading && (
              <tr>
                <td colSpan={columns.length} className="text-center py-8 text-muted">
                  Loading…
                </td>
              </tr>
            )}
            {!loading && paged.length === 0 && (
              <tr>
                <td colSpan={columns.length}>{empty ?? <div className="text-center py-8 text-muted">No data</div>}</td>
              </tr>
            )}
            {!loading &&
              paged.map((row) => (
                <tr
                  key={rowKey(row)}
                  onClick={onRowClick ? () => onRowClick(row) : undefined}
                  className={cn(
                    'border-b border-border last:border-0 transition-colors',
                    onRowClick && 'cursor-pointer hover:bg-surface-hover',
                  )}
                >
                  {columns.map((c) => (
                    <td
                      key={c.key}
                      className={cn(
                        'px-3 py-2',
                        c.align === 'right' && 'text-right',
                        c.align === 'center' && 'text-center',
                      )}
                    >
                      {c.cell(row)}
                    </td>
                  ))}
                </tr>
              ))}
          </tbody>
        </table>
      </div>
      {pageSize > 0 && totalPages > 1 && (
        <div className="flex items-center justify-between px-3 py-2 border-t border-border text-[11px] text-muted">
          <span>
            Page {page + 1} of {totalPages} · {sorted.length} rows
          </span>
          <div className="flex gap-1">
            <button
              onClick={() => setPage((p) => Math.max(0, p - 1))}
              disabled={page === 0}
              className="px-2 py-0.5 rounded border border-border bg-surface-2 disabled:opacity-40"
            >
              Prev
            </button>
            <button
              onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
              disabled={page >= totalPages - 1}
              className="px-2 py-0.5 rounded border border-border bg-surface-2 disabled:opacity-40"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
