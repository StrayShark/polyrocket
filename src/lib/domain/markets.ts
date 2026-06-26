/**
 * L1 领域镜像 —— M1 市场。
 * 镜像 src-tauri/src/domain/polymarket/mod.rs。
 * 纯函数，无 IO，可安全地从 L1 路由中导入。
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

/** Category → string。直接返回枚举值（TS 字符串字面量本身已经显式） */
export function categoryAsString(c: Category): string {
  return c;
}

/**
 * Best-effort 把一段自由格式的问题文本分类到一个 category。
 * 小写子串匹配，第一个匹配胜出。
 */
/** 把自由文本 market 问题分类到 9 个 `Category` 之一。**best-effort**：lowercase substring match，第一个匹配胜。
 *
 * **为什么是 L1 函数而不是 Rust**：Polymarket 的 market 列表（L1 拉）没有 category
 * 字段；需要 L1 自己做分类用于 UI filter / sort。失败时 fallback `'other'`。
 *
 * @param question — market question 文本
 * @returns 分类 enum 值
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

/** 把 liquidity decimal string 解析为 number。null / undefined / NaN → 0.
 *
 * @returns liquidity（USDC，0 if invalid）
 */
export function parseLiquidity(s: string | null | undefined): number {
  if (s == null) return 0;
  const n = Number(s);
  return Number.isFinite(n) ? n : 0;
}

export function parseVolume24h(s: string | null | undefined): number {
  return parseLiquidity(s);
}

/** 算 market 距关闭还有多少小时。**向下取整**。
 *
 * **负数**：market 已过期（end_date < now）。
 * **0**：正好关闭（end_date = now）→ floor 0。
 *
 * @returns 小时数
 */
export function hoursUntilClose(endDateMs: number, nowMs: number): number {
  return Math.floor((endDateMs - nowMs) / 3_600_000);
}

/** 算 market 是否在 `horizonHours` 内关闭。**不含**已经关闭的（dt <= 0 返回 false）。
 *
 * @returns true 表示「未来 horizon 小时内关闭」
 */
export function closesWithin(marketEndMs: number, nowMs: number, horizonHours: number): boolean {
  const dt = marketEndMs - nowMs;
  return dt > 0 && dt <= horizonHours * 3_600_000;
}

/** 把 market 关闭时间分类到 UI 显示桶。
 *
 * **桶**：`'closed'` / `'< 1h'` / `'today'` / `'this week'` / `'this month'` / `'later'`。
 * L1 「Markets」列表用这个分组显示。
 */
export function closingBucket(endDateMs: number, nowMs: number): string {
  const h = hoursUntilClose(endDateMs, nowMs);
  if (h < 0) return 'closed';
  if (h < 1) return '< 1h';
  if (h < 24) return 'today';
  if (h < 24 * 7) return 'this week';
  if (h < 24 * 30) return 'this month';
  return 'later';
}
