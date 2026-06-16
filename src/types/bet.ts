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

export interface PlaceJumpArgs {
  market_slug: string;
  market_id: string;
  wallet_id: string;
  side: BetSide;
  size: string;
  price: number;
  signal_id?: number;
}

export interface PlaceSignedArgs {
  market_id: string;
  wallet_id: string;
  side: BetSide;
  price: number;
  size: string;
  signal_id?: number;
  /** Alias of the key stored in OS keyring (e.g. "primary", "trade-1") */
  key_alias: string;
}

export interface ListBetsArgs {
  status?: BetStatus;
  wallet_id?: string;
  limit?: number;
}
