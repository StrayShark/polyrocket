// Bet DTOs (mirror src-tauri/src/commands/bet.rs + domain::bet)

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
}

// v0.44c — paper fill DTO. The mirror of a Bet
// minus tx_hash, plus an explicit mirror_id link.
// Used by listPaperFills and the [PAPER] badges
// in Copy / PnL pages.
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

// v0.50a — order types. Mirrors src-tauri OrderType.
export type OrderType = 'market' | 'limit' | 'stop_loss';

export interface PlaceSignedArgs {
  market_id: string;
  wallet_id: string;
  side: BetSide;
  price: number;
  size: string;
  signal_id?: number;
  /** Alias of the key stored in OS keyring (e.g. "primary", "trade-1") */
  key_alias: string;
  /** v0.50a — order type. Defaults to 'market' when
   * omitted (back-compat for pre-v0.50 call sites). */
  order_type?: OrderType;
  /** v0.50a — limit price (required for 'limit' orders;
   * optional for 'stop_loss', defaults to stop_price). */
  limit_price?: number;
  /** v0.50a — stop price (required for 'stop_loss'). */
  stop_price?: number;
  /** v0.50b — post-only flag (limit orders only). */
  post_only?: boolean;
}

export interface ListBetsArgs {
  status?: BetStatus;
  wallet_id?: string;
  limit?: number;
}

/** v0.50a — args for `validateOrderArgs`. Mirrors
 * `commands::bet::ValidateOrderArgsArgs`. The L1
 * calls this for preflight validation before
 * invoking `placeSignedOrder`. */
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
