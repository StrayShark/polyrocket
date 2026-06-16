// Signal DTOs (mirror src-tauri/src/commands/signal.rs + domain::signal)

export interface Signal {
  id: number;
  market_id: string;
  computed_at: number;
  model_version: string;
  predicted_prob: number;
  market_prob: number;
  edge: number;
  confidence: number;
  horizon_hours: number;
  rationale: string | null;
  market_question?: string | null;
  market_slug?: string | null;
}

export interface ListSignalsArgs {
  min_edge?: number;
  category?: string;
  limit?: number;
}
