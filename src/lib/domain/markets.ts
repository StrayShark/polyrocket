/**
 * L1 domain mirror — M1 Markets.
 * Mirrors src-tauri/src/domain/polymarket/mod.rs.
 * Pure functions, no IO. Safe to import from L1 routes.
 */

export type Category =
  | 'football'
  | 'cs2'
  | 'politics'
  | 'crypto'
  | 'tech'
  | 'science'
  | 'pop-culture'
  | 'business'
  | 'other';

export const CATEGORY_LABELS: Record<Category, string> = {
  football: 'Football',
  cs2: 'CS2',
  politics: 'Politics',
  crypto: 'Crypto',
  tech: 'Tech',
  science: 'Science',
  'pop-culture': 'Pop Culture',
  business: 'Business',
  other: 'Other',
};

export function categoryAsString(c: Category): string {
  return c;
}

/**
 * Best-effort classify a free-form question string into a category.
 * Lowercased substring match. First match wins.
 */
export function classifyCategory(question: string): Category {
  const q = question.toLowerCase();
  if (
    q.includes('fc ') ||
    q.includes(' vs ') ||
    q.includes('football') ||
    q.includes('nba') ||
    q.includes('nfl') ||
    q.includes('premier league')
  ) {
    return 'football';
  }
  if (q.includes('cs2') || q.includes('counter-strike') || q.includes('esl') || q.includes('blast')) {
    return 'cs2';
  }
  if (
    q.includes('trump') ||
    q.includes('biden') ||
    q.includes('election') ||
    q.includes('president') ||
    q.includes('senate') ||
    q.includes('congress')
  ) {
    return 'politics';
  }
  if (q.includes('bitcoin') || q.includes('btc') || q.includes('eth') || q.includes('crypto')) {
    return 'crypto';
  }
  if (q.includes('ai ') || q.includes('openai') || q.includes('gpt') || q.includes('anthropic') || q.includes('llm')) {
    return 'tech';
  }
  if (q.includes('nasa') || q.includes('spacex') || q.includes('research')) {
    return 'science';
  }
  if (q.includes('movie') || q.includes('oscar') || q.includes('grammy') || q.includes('celebrity')) {
    return 'pop-culture';
  }
  if (q.includes('stock') || q.includes('market cap') || q.includes('ipo') || q.includes('revenue')) {
    return 'business';
  }
  return 'other';
}

export function parseLiquidity(s: string | null | undefined): number {
  if (s == null) return 0;
  const n = Number(s);
  return Number.isFinite(n) ? n : 0;
}

export function parseVolume24h(s: string | null | undefined): number {
  return parseLiquidity(s);
}

export function hoursUntilClose(endDateMs: number, nowMs: number): number {
  return Math.floor((endDateMs - nowMs) / 3_600_000);
}

export function closesWithin(marketEndMs: number, nowMs: number, horizonHours: number): boolean {
  const dt = marketEndMs - nowMs;
  return dt > 0 && dt <= horizonHours * 3_600_000;
}

export function closingBucket(endDateMs: number, nowMs: number): string {
  const h = hoursUntilClose(endDateMs, nowMs);
  if (h < 0) return 'closed';
  if (h < 1) return '< 1h';
  if (h < 24) return 'today';
  if (h < 24 * 7) return 'this week';
  if (h < 24 * 30) return 'this month';
  return 'later';
}
