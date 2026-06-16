CREATE TABLE `audit_log` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`at` integer DEFAULT (unixepoch() * 1000) NOT NULL,
	`actor` text NOT NULL,
	`action` text NOT NULL,
	`target` text,
	`payload` text,
	`result` text
);
--> statement-breakpoint
CREATE INDEX `audit_at_idx` ON `audit_log` (`at`);--> statement-breakpoint
CREATE TABLE `bets` (
	`id` text PRIMARY KEY NOT NULL,
	`wallet_id` text NOT NULL,
	`market_id` text NOT NULL,
	`signal_id` integer,
	`decision_id` integer,
	`was_llm_assisted` integer DEFAULT false NOT NULL,
	`mode` text NOT NULL,
	`side` text NOT NULL,
	`size` text NOT NULL,
	`price` real NOT NULL,
	`shares` text NOT NULL,
	`placed_at` integer NOT NULL,
	`settled_at` integer,
	`pnl` text,
	`status` text NOT NULL,
	`tx_hash` text,
	`notes` text,
	FOREIGN KEY (`wallet_id`) REFERENCES `wallets`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`market_id`) REFERENCES `markets`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`signal_id`) REFERENCES `signals`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`decision_id`) REFERENCES `llm_decisions`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `bets_wallet_idx` ON `bets` (`wallet_id`,`placed_at`);--> statement-breakpoint
CREATE INDEX `bets_market_idx` ON `bets` (`market_id`,`placed_at`);--> statement-breakpoint
CREATE INDEX `bets_status_idx` ON `bets` (`status`);--> statement-breakpoint
CREATE TABLE `copy_events` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`target_id` text NOT NULL,
	`market_id` text NOT NULL,
	`detected_at` integer NOT NULL,
	`side` text NOT NULL,
	`size` text NOT NULL,
	`price` real NOT NULL,
	`tx_hash` text NOT NULL,
	`matched_bet_id` text,
	FOREIGN KEY (`target_id`) REFERENCES `copy_targets`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`market_id`) REFERENCES `markets`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`matched_bet_id`) REFERENCES `bets`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE UNIQUE INDEX `copy_events_tx_hash_unique` ON `copy_events` (`tx_hash`);--> statement-breakpoint
CREATE INDEX `copy_events_target_time_idx` ON `copy_events` (`target_id`,`detected_at`);--> statement-breakpoint
CREATE TABLE `copy_targets` (
	`id` text PRIMARY KEY NOT NULL,
	`address` text NOT NULL,
	`label` text,
	`enabled` integer DEFAULT true NOT NULL,
	`allocation_cap` text,
	`min_edge` real DEFAULT 0.05 NOT NULL,
	`created_at` integer DEFAULT (unixepoch() * 1000) NOT NULL
);
--> statement-breakpoint
CREATE UNIQUE INDEX `copy_targets_address_unique` ON `copy_targets` (`address`);--> statement-breakpoint
CREATE INDEX `copy_addr_idx` ON `copy_targets` (`address`);--> statement-breakpoint
CREATE TABLE `daily_briefs` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`market_id` text NOT NULL,
	`rank` integer NOT NULL,
	`match_score` real NOT NULL,
	`score_breakdown` text,
	`computed_at` integer NOT NULL,
	`expires_at` integer NOT NULL,
	FOREIGN KEY (`market_id`) REFERENCES `markets`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `daily_briefs_rank_idx` ON `daily_briefs` (`rank`,`computed_at`);--> statement-breakpoint
CREATE INDEX `daily_briefs_market_idx` ON `daily_briefs` (`market_id`,`computed_at`);--> statement-breakpoint
CREATE TABLE `llm_analyses` (
	`id` text PRIMARY KEY NOT NULL,
	`market_id` text NOT NULL,
	`signal_id` integer,
	`prompt_version` text NOT NULL,
	`requested_at` integer NOT NULL,
	`completed_at` integer,
	`status` text NOT NULL,
	`consensus_predicted` real,
	`consensus_side` text,
	`consensus_conf` real,
	`total_latency_ms` integer,
	`cost_cents` real,
	`triggered_by` text NOT NULL,
	FOREIGN KEY (`market_id`) REFERENCES `markets`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`signal_id`) REFERENCES `signals`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `analyses_market_time_idx` ON `llm_analyses` (`market_id`,`requested_at`);--> statement-breakpoint
CREATE INDEX `analyses_status_idx` ON `llm_analyses` (`status`);--> statement-breakpoint
CREATE TABLE `llm_call_logs` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`analysis_id` text,
	`provider_id` text NOT NULL,
	`key_id` text,
	`called_at` integer NOT NULL,
	`latency_ms` integer NOT NULL,
	`tokens_in` integer DEFAULT 0 NOT NULL,
	`tokens_out` integer DEFAULT 0 NOT NULL,
	`cost_cents` real DEFAULT 0 NOT NULL,
	`http_status` integer NOT NULL,
	`success` integer NOT NULL,
	`error_code` text,
	`error_message` text,
	`prompt_version` text,
	`predicted_prob` real,
	`recommended_side` text,
	`caller` text NOT NULL,
	`retry_count` integer DEFAULT 0 NOT NULL,
	FOREIGN KEY (`provider_id`) REFERENCES `llm_providers`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`key_id`) REFERENCES `llm_provider_keys`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `call_logs_provider_time_idx` ON `llm_call_logs` (`provider_id`,`called_at`);--> statement-breakpoint
CREATE INDEX `call_logs_analysis_idx` ON `llm_call_logs` (`analysis_id`);--> statement-breakpoint
CREATE INDEX `call_logs_success_time_idx` ON `llm_call_logs` (`success`,`called_at`);--> statement-breakpoint
CREATE TABLE `llm_decisions` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`analysis_id` text NOT NULL,
	`bet_id` text,
	`user_decision` text NOT NULL,
	`user_decided_side` text,
	`followed_llm_id` integer,
	`decided_at` integer NOT NULL,
	`context_snapshot` text,
	FOREIGN KEY (`analysis_id`) REFERENCES `llm_analyses`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`bet_id`) REFERENCES `bets`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`followed_llm_id`) REFERENCES `llm_recommendations`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `decisions_analysis_idx` ON `llm_decisions` (`analysis_id`);--> statement-breakpoint
CREATE INDEX `decisions_bet_idx` ON `llm_decisions` (`bet_id`);--> statement-breakpoint
CREATE TABLE `llm_health_checks` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`provider_id` text NOT NULL,
	`key_id` text,
	`checked_at` integer NOT NULL,
	`trigger` text NOT NULL,
	`success` integer NOT NULL,
	`latency_ms` integer,
	`http_status` integer,
	`error_code` text,
	`error_message` text,
	`model_used` text,
	`test_request_id` text,
	FOREIGN KEY (`provider_id`) REFERENCES `llm_providers`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`key_id`) REFERENCES `llm_provider_keys`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `health_provider_time_idx` ON `llm_health_checks` (`provider_id`,`checked_at`);--> statement-breakpoint
CREATE INDEX `health_success_time_idx` ON `llm_health_checks` (`success`,`checked_at`);--> statement-breakpoint
CREATE TABLE `llm_provider_keys` (
	`id` text PRIMARY KEY NOT NULL,
	`provider_id` text NOT NULL,
	`alias` text NOT NULL,
	`keyring_alias` text NOT NULL,
	`enabled` integer DEFAULT true NOT NULL,
	`priority` integer DEFAULT 0 NOT NULL,
	`weight` integer DEFAULT 1 NOT NULL,
	`last_used_at` integer,
	`last_error` text,
	`last_error_at` integer,
	`total_calls` integer DEFAULT 0 NOT NULL,
	`total_errors` integer DEFAULT 0 NOT NULL,
	`notes` text,
	`created_at` integer DEFAULT (unixepoch() * 1000) NOT NULL,
	`updated_at` integer DEFAULT (unixepoch() * 1000) NOT NULL,
	FOREIGN KEY (`provider_id`) REFERENCES `llm_providers`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `llm_keys_provider_idx` ON `llm_provider_keys` (`provider_id`,`priority`);--> statement-breakpoint
CREATE TABLE `llm_providers` (
	`id` text PRIMARY KEY NOT NULL,
	`display_name` text NOT NULL,
	`provider_kind` text DEFAULT 'openai' NOT NULL,
	`request_format` text DEFAULT 'chat_completions' NOT NULL,
	`supports_streaming` integer DEFAULT false NOT NULL,
	`enabled` integer DEFAULT true NOT NULL,
	`api_base` text,
	`key_alias` text NOT NULL,
	`default_model` text NOT NULL,
	`timeout_ms` integer DEFAULT 30000 NOT NULL,
	`request_timeout_ms` integer DEFAULT 30000 NOT NULL,
	`max_retries` integer DEFAULT 2 NOT NULL,
	`cost_per_1k_in` real,
	`cost_per_1k_out` real,
	`rate_limit_rpm` integer,
	`rate_limit_tpm` integer,
	`quota_daily_cents` real,
	`quota_monthly_cents` real,
	`key_rotation_strategy` text DEFAULT 'failover' NOT NULL,
	`health_status` text DEFAULT 'unknown' NOT NULL,
	`health_latency_p50_ms` integer,
	`health_latency_p95_ms` integer,
	`last_health_check_at` integer,
	`last_health_error` text,
	`notes` text,
	`created_at` integer DEFAULT (unixepoch() * 1000) NOT NULL,
	`updated_at` integer DEFAULT (unixepoch() * 1000) NOT NULL
);
--> statement-breakpoint
CREATE TABLE `llm_recommendations` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`analysis_id` text NOT NULL,
	`provider_id` text NOT NULL,
	`predicted_prob` real,
	`side` text,
	`confidence` real,
	`reasoning` text,
	`latency_ms` integer,
	`tokens_in` integer,
	`tokens_out` integer,
	`cost_cents` real,
	`raw_response` text,
	`parse_ok` integer NOT NULL,
	`parse_error` text,
	`created_at` integer DEFAULT (unixepoch() * 1000) NOT NULL,
	FOREIGN KEY (`analysis_id`) REFERENCES `llm_analyses`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`provider_id`) REFERENCES `llm_providers`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `recs_analysis_idx` ON `llm_recommendations` (`analysis_id`);--> statement-breakpoint
CREATE INDEX `recs_provider_idx` ON `llm_recommendations` (`provider_id`);--> statement-breakpoint
CREATE TABLE `markets` (
	`id` text PRIMARY KEY NOT NULL,
	`slug` text NOT NULL,
	`question` text NOT NULL,
	`description` text,
	`category` text NOT NULL,
	`tags` text,
	`end_date` integer NOT NULL,
	`active` integer DEFAULT true NOT NULL,
	`resolved` integer DEFAULT false NOT NULL,
	`outcome` text,
	`liquidity` text,
	`volume_24h` text,
	`user_interested` integer DEFAULT false NOT NULL,
	`brief_dismissed_at` integer,
	`created_at` integer DEFAULT (unixepoch() * 1000) NOT NULL,
	`updated_at` integer DEFAULT (unixepoch() * 1000) NOT NULL
);
--> statement-breakpoint
CREATE UNIQUE INDEX `markets_slug_unique` ON `markets` (`slug`);--> statement-breakpoint
CREATE INDEX `markets_cat_idx` ON `markets` (`category`,`active`);--> statement-breakpoint
CREATE INDEX `markets_end_idx` ON `markets` (`end_date`);--> statement-breakpoint
CREATE TABLE `model_performance` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`model_version` text NOT NULL,
	`category` text,
	`window_start` integer NOT NULL,
	`window_end` integer NOT NULL,
	`n_predictions` integer NOT NULL,
	`brier_score` real NOT NULL,
	`log_loss` real,
	`win_rate` real,
	`avg_edge` real,
	`calibration` text
);
--> statement-breakpoint
CREATE INDEX `perf_model_window_idx` ON `model_performance` (`model_version`,`window_end`);--> statement-breakpoint
CREATE TABLE `orderbook_snapshots` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`market_id` text NOT NULL,
	`captured_at` integer NOT NULL,
	`best_bid` real NOT NULL,
	`best_ask` real NOT NULL,
	`mid_price` real NOT NULL,
	`spread` real NOT NULL,
	`bid_liquidity` text,
	`ask_liquidity` text,
	FOREIGN KEY (`market_id`) REFERENCES `markets`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `snapshots_market_time_idx` ON `orderbook_snapshots` (`market_id`,`captured_at`);--> statement-breakpoint
CREATE TABLE `signals` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`market_id` text NOT NULL,
	`computed_at` integer NOT NULL,
	`model_version` text NOT NULL,
	`predicted_prob` real NOT NULL,
	`market_prob` real NOT NULL,
	`edge` real NOT NULL,
	`confidence` real NOT NULL,
	`horizon_hours` integer NOT NULL,
	`rationale` text,
	`active` integer DEFAULT true NOT NULL,
	FOREIGN KEY (`market_id`) REFERENCES `markets`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `signals_market_active_idx` ON `signals` (`market_id`,`active`);--> statement-breakpoint
CREATE INDEX `signals_edge_idx` ON `signals` (`edge`);--> statement-breakpoint
CREATE TABLE `ticks` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`market_id` text NOT NULL,
	`captured_at` integer NOT NULL,
	`price` real NOT NULL,
	`side` text NOT NULL,
	`size` text,
	FOREIGN KEY (`market_id`) REFERENCES `markets`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE INDEX `ticks_market_time_idx` ON `ticks` (`market_id`,`captured_at`);--> statement-breakpoint
CREATE TABLE `user_brief_prefs` (
	`user_id` text PRIMARY KEY NOT NULL,
	`weights_json` text NOT NULL,
	`max_items` integer DEFAULT 5 NOT NULL,
	`min_liquidity` text,
	`categories` text,
	`updated_at` integer DEFAULT (unixepoch() * 1000) NOT NULL
);
--> statement-breakpoint
CREATE TABLE `wallets` (
	`id` text PRIMARY KEY NOT NULL,
	`address` text NOT NULL,
	`label` text,
	`chain_id` integer DEFAULT 137 NOT NULL,
	`wallet_type` text NOT NULL,
	`created_at` integer DEFAULT (unixepoch() * 1000) NOT NULL,
	`last_synced_at` integer
);
--> statement-breakpoint
CREATE UNIQUE INDEX `wallets_address_unique` ON `wallets` (`address`);--> statement-breakpoint
CREATE INDEX `wallets_addr_idx` ON `wallets` (`address`);