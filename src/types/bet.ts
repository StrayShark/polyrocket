// Bet DTO（镜像 src-tauri/src/commands/bet.rs + domain::bet）

export type BetMode = 'A_jump' | 'B_signed';
export type BetStatus = 'open' | 'won' | 'lost' | 'cancelled';
export type BetSide = 'YES' | 'NO';

export interface Bet {
  id: string;
  wallet_id: string;
  market_id: string;
  signal_id: number | null;
  mode: BetMode;
  side: BetSide;
  size: string;
  price: number;
  shares: string;
  placed_at: number;
  settled_at: number | null;
  pnl: string | null;
  status: BetStatus;
  tx_hash: string | null;
  notes: string | null;
  // v0.50a —— 订单类型（在 Rust 端
  // 默认为 'market' 以保持向后兼容）。
  order_type?: OrderType;
  // v0.50a —— 限价 / 止损价（仅对
  // 非市价单设置）。
  limit_price?: number | null;
  stop_price?: number | null;
  // v0.50b —— post-only 标记。
  post_only?: boolean;
  // v0.51b —— 由提交后的对账步骤
  // （真实 HTTP 路径或 stub 回退）
  // 填充的成交字段。
  filled_at?: number | null;
  fill_price?: number | null;
  fill_size?: string | null;
  partial?: boolean;
}

// v0.44c —— paper fill DTO。Bet 的镜像
// 减去 tx_hash，再加上一个显式 mirror_id 链接。
// 供 listPaperFills 及 Copy / PnL 页面中的
// [PAPER] 标签使用。
export interface PaperFill {
  id: string;
  mirror_id: string;
  market_id: string;
  side: BetSide;
  size: string;
  price: number;
  placed_at: number;
  notes: string | null;
}

export interface PlaceJumpArgs {
  market_slug: string;
  market_id: string;
  wallet_id: string;
  side: BetSide;
  size: string;
  price: number;
  signal_id?: number;
}

// v0.50a —— 订单类型。镜像 src-tauri OrderType。
export type OrderType = 'market' | 'limit' | 'stop_loss';

export interface PlaceSignedArgs {
  market_id: string;
  wallet_id: string;
  side: BetSide;
  price: number;
  size: string;
  signal_id?: number;
  /** OS keyring 中存储的 key 别名（例如 "primary"、"trade-1"） */
  key_alias: string;
  /** v0.50a —— 订单类型。省略时默认为
   * 'market'（对 v0.50 之前的调用方保持向后兼容）。 */
  order_type?: OrderType;
  /** v0.50a —— 限价（'limit' 订单必填；
   * 'stop_loss' 可选，默认为 stop_price）。 */
  limit_price?: number;
  /** v0.50a —— 止损价（'stop_loss' 必填）。 */
  stop_price?: number;
  /** v0.50b —— post-only 标记（仅限价单）。 */
  post_only?: boolean;
}

export interface ListBetsArgs {
  status?: BetStatus;
  wallet_id?: string;
  limit?: number;
}

/** v0.50a —— `validateOrderArgs` 的参数。
 * 镜像 `commands::bet::ValidateOrderArgsArgs`。
 * L1 在调用 `placeSignedOrder` 之前
 * 进行预校验时会调用它。 */
export interface ValidateOrderArgsArgs {
  market_id: string;
  side: BetSide;
  size: string;
  price: number;
  order_type?: OrderType;
  limit_price?: number;
  stop_price?: number;
  post_only?: boolean;
}
