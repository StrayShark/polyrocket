/** Convert a value to a CSV cell string. */
function cell(v: unknown): string {
  if (v === null || v === undefined) return '';
  const s = typeof v === 'string' ? v : String(v);
  if (s.includes(',') || s.includes('"') || s.includes('\n')) {
    return `"${s.replace(/"/g, '""')}"`;
  }
  return s;
}

/**
 * Convert an array of plain objects to a CSV string. Keys of the
 * first row determine column order. Numbers, booleans, and nulls
 * are coerced via String(). Use lib/format.ts for human-readable
 * display — this is for export only.
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

/** Trigger a browser download of the CSV string. */
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
