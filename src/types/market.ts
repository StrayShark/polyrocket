// Market DTO（镜像 src-tauri/src/commands/market.rs + domain::polymarket）

/** L1 Market DTO。**镜像** `src-tauri/src/commands/market.rs::MarketDto`。
 *
 * **wire format**：`snake_case` 字段名（跟 Rust DTO 一致）。Tauri 2 IPC
 * 自动转译 args；L1 直接拿到 snake_case JSON。
 *
 * **USDC 字段**：`liquidity` / `volume_24h` 都是 decimal string（避免精度损失）。
 * L1 用 `parseFloat` 渲染。
 *
 * **optional 字段**：
 *   - `user_interested` — v0.3+ L1 dashboard「I'm interested」按钮
 *   - `brief_dismissed_at` — v0.3+ 简报 dismiss 时间戳
 */
export interface Market {
  id: string;
  slug: string;
  question: string;
  category: string;
  end_date: number;
  active: boolean;
  resolved: boolean;
  outcome: string | null;
  liquidity: string | null;
  volume_24h: string | null;
  /** user_interested (v0.3+, optional,是否感兴趣) */
  user_interested?: number | null;
  /** brief_dismissed_at (v0.3+, optional,简报 dismiss 时间戳) */
  brief_dismissed_at?: number | null;
}

/** `list_markets` IPC 参数。L1 「Markets」页面 mount 时调。
 *
 * **filter**：
 *   - `category` — 精确匹配
 *   - `active_only` — true 排除已 resolve 的
 *   - `limit` — 默认 100，最大 500
 */
export interface ListMarketsArgs {
  category?: string;
  active_only?: boolean;
  limit?: number;
}

/** Market 轻量版 DTO。L1 「Daily Brief」卡片展示用（不含 outcome / liquidity detail）。
 *
 * **跟 Market 的差别**：字段全 9 个，**不**含 `user_interested` / `brief_dismissed_at`。
 * Daily Brief 不需要这两个状态字段。
 */
export interface MarketSummary {
  id: string;
  slug: string;
  question: string;
  category: string;
  end_date: number;
  active: boolean;
  resolved: boolean;
  outcome: string | null;
  liquidity: string | null;
  volume_24h: string | null;
}
