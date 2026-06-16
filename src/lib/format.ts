/** Format helpers — numbers, percentages, currency, time, addresses. */

const NF_USDC = new Intl.NumberFormat('en-US', {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

const NF_PCT = new Intl.NumberFormat('en-US', {
  style: 'percent',
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});

const NF_PCT_INT = new Intl.NumberFormat('en-US', {
  style: 'percent',
  maximumFractionDigits: 0,
});

export const fmtUsdc = (s: string | number | null | undefined): string => {
  if (s === null || s === undefined) return '—';
  const n = typeof s === 'string' ? Number(s) : s;
  if (!Number.isFinite(n)) return '—';
  return NF_USDC.format(n);
};

export const fmtPct = (n: number | null | undefined, signed = false): string => {
  if (n === null || n === undefined || !Number.isFinite(n)) return '—';
  const sign = signed && n > 0 ? '+' : '';
  return sign + NF_PCT.format(n);
};

export const fmtPctInt = (n: number | null | undefined): string => {
  if (n === null || n === undefined || !Number.isFinite(n)) return '—';
  return NF_PCT_INT.format(n);
};

export const fmtEdge = (n: number | null | undefined): string => {
  if (n === null || n === undefined) return '—';
  return (n >= 0 ? '+' : '') + NF_PCT.format(n);
};

export const fmtConfidence = (n: number | null | undefined): string => {
  if (n === null || n === undefined) return '—';
  return NF_PCT.format(n);
};

export const fmtLatency = (ms: number | null | undefined): string => {
  if (ms === null || ms === undefined) return '—';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
};

export const fmtCents = (c: number | null | undefined): string => {
  if (c === null || c === undefined) return '—';
  return `$${NF_USDC.format(c / 100)}`;
};

export const fmtAddress = (addr: string | null | undefined, head = 6, tail = 4): string => {
  if (!addr) return '—';
  if (addr.length <= head + tail + 1) return addr;
  return `${addr.slice(0, head)}…${addr.slice(-tail)}`;
};

export const fmtRelativeTime = (ms: number | null | undefined): string => {
  if (ms === null || ms === undefined) return '—';
  const diff = Date.now() - ms;
  const abs = Math.abs(diff);
  const sign = diff >= 0 ? '' : 'in ';
  const suffix = diff >= 0 ? ' ago' : '';
  if (abs < 60_000) return sign + 'just now'.replace('in ', 'now');
  if (abs < 3_600_000) return `${sign}${Math.floor(abs / 60_000)}m${suffix}`;
  if (abs < 86_400_000) return `${sign}${Math.floor(abs / 3_600_000)}h${suffix}`;
  return `${sign}${Math.floor(abs / 86_400_000)}d${suffix}`;
};

export const fmtDate = (ms: number | null | undefined): string => {
  if (ms === null || ms === undefined) return '—';
  return new Date(ms).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
};

export const fmtDateTime = (ms: number | null | undefined): string => {
  if (ms === null || ms === undefined) return '—';
  return new Date(ms).toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
};
