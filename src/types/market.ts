// Market DTOs (mirror src-tauri/src/commands/market.rs + domain::polymarket)

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
  /** user_interested (v0.3+, optional) */
  user_interested?: number | null;
  /** brief_dismissed_at (v0.3+, optional) */
  brief_dismissed_at?: number | null;
}

export interface ListMarketsArgs {
  category?: string;
  active_only?: boolean;
  limit?: number;
}

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
