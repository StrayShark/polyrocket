/** 将一个值转换为 CSV 单元格字符串。 */
function cell(v: unknown): string {
  if (v === null || v === undefined) return '';
  const s = typeof v === 'string' ? v : String(v);
  if (s.includes(',') || s.includes('"') || s.includes('\n')) {
    return `"${s.replace(/"/g, '""')}"`;
  }
  return s;
}

/**
 * 将一个普通对象数组转换为 CSV 字符串。
 * 第一行的 keys 决定列顺序。
 * 数字、布尔值和 null 通过 String()
 * 强制转换。人类可读的展示请使用
 * lib/format.ts —— 此函数仅用于导出。
 */
export function toCsv<T extends Record<string, unknown>>(
  rows: T[],
  columns?: (keyof T)[],
): string {
  if (rows.length === 0) return '';
  const cols = (columns ?? (Object.keys(rows[0]) as (keyof T)[])) as (keyof T)[];
  const header = cols.map(String).join(',');
  const body = rows
    .map((r) => cols.map((c) => cell(r[c])).join(','))
    .join('\n');
  return `${header}\n${body}\n`;
}

/** 触发浏览器下载 CSV 字符串。 */
export function downloadCsv(filename: string, csv: string): void {
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
