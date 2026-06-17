//! L3 — Demo data seeder (pure).
//!
//! Deterministic sample data so a fresh `cargo tauri dev` shows a
//! fully-populated UI (Dashboard charts, History rows, P&L signals,
//! Copy tracker rows, …) instead of an empty shell.
//!
//! Constraints:
//!   - All IDs are stable strings (no uuid, no random) so screenshots
//!     and integration tests are reproducible.
//!   - All timestamps are pinned to `SEED_NOW_MS` (a fixed epoch).
//!     Re-running the seeder produces the same byte-identical rows.
//!   - Quantities are decimal strings (e.g. "12.50") matching the
//!     schema's `text` representation for `size`/`shares`/`liquidity`.
//!
//! Trigger: `infra::db::seed::apply_seed` (idempotent) is called from
//! `init_pool` on first launch and on explicit `seed_demo_data(force=true)`.

use serde::{Deserialize, Serialize};

/// The fixed reference epoch for all seeded timestamps.
/// 2026-01-15 12:00:00 UTC, in milliseconds.
pub const SEED_NOW_MS: i64 = 1_768_456_800_000;

/// Marker string written to `audit_log.action` so the seeder is
/// observable in the Audit page.
pub const SEED_AUDIT_ACTION: &str = "seed_demo_data";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedWallet {
    pub id: String,
    pub address: String,
    pub label: String,
    pub chain_id: i64,
    pub wallet_type: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedMarket {
    pub id: String,
    pub slug: String,
    pub question: String,
    pub description: String,
    pub category: String,
    pub tags: String,        // comma-separated, e.g. "nba,playoffs"
    pub end_date: i64,
    pub active: bool,
    pub resolved: bool,
    pub outcome: Option<String>,
    pub liquidity: Option<String>,
    pub volume_24h: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedSignal {
    pub market_id: String,
    pub computed_at: i64,
    pub model_version: String,
    pub predicted_prob: f64,
    pub market_prob: f64,
    pub edge: f64,
    pub confidence: f64,
    pub horizon_hours: i64,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedBet {
    pub id: String,
    pub wallet_id: String,
    pub market_id: String,
    pub signal_id: Option<i64>,
    pub decision_id: Option<i64>,
    pub was_llm_assisted: bool,
    pub mode: String,            // "jump" | "signed"
    pub side: String,            // "yes" | "no"
    pub size: String,            // decimal string e.g. "12.50"
    pub price: f64,              // 0..1
    pub shares: String,
    pub placed_at: i64,
    pub settled_at: Option<i64>,
    pub pnl: Option<String>,
    pub status: String,          // "open" | "settled_yes" | "settled_no" | "cancelled"
    pub tx_hash: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedCopyTarget {
    pub id: String,
    pub address: String,
    pub label: String,
    pub enabled: bool,
    pub allocation_cap: String,
    pub min_edge: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedCopyEvent {
    pub target_id: String,
    pub market_id: String,
    pub detected_at: i64,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub tx_hash: String,
    pub matched_bet_id: Option<String>,
}

/// All seed rows. Functions return `Vec<...>` and are pure — no I/O.
#[derive(Debug, Clone, Default)]
pub struct SeedBundle {
    pub wallets: Vec<SeedWallet>,
    pub markets: Vec<SeedMarket>,
    pub signals: Vec<SeedSignal>,
    pub bets: Vec<SeedBet>,
    pub copy_targets: Vec<SeedCopyTarget>,
    pub copy_events: Vec<SeedCopyEvent>,
}

impl SeedBundle {
    /// Return the canonical demo dataset.
    /// 4 wallets · 12 markets · 14 signals · 18 bets · 3 copy targets · 5 copy events
    pub fn demo() -> Self {
        let wallets = seed_wallets();
        let markets = seed_markets();
        let signals = seed_signets(&markets);
        let bets = seed_bets(&wallets, &markets, &signals);
        let copy_targets = seed_copy_targets();
        let copy_events = seed_copy_events(&copy_targets, &markets, &bets);
        Self { wallets, markets, signals, bets, copy_targets, copy_events }
    }

    /// Total row count — useful for the IPC return value.
    pub fn total_rows(&self) -> usize {
        self.wallets.len()
            + self.markets.len()
            + self.signals.len()
            + self.bets.len()
            + self.copy_targets.len()
            + self.copy_events.len()
    }
}

fn seed_wallets() -> Vec<SeedWallet> {
    vec![
        SeedWallet {
            id: "wallet-demo-1".into(),
            address: "0xDemo1aA1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1".into(),
            label: "primary · read-only".into(),
            chain_id: 137,
            wallet_type: "browser".into(),
            created_at: SEED_NOW_MS - 30 * 86_400_000, // 30 days ago
        },
        SeedWallet {
            id: "wallet-demo-2".into(),
            address: "0xDemo2bB2B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2".into(),
            label: "trading · Mode B".into(),
            chain_id: 137,
            wallet_type: "browser".into(),
            created_at: SEED_NOW_MS - 21 * 86_400_000,
        },
        SeedWallet {
            id: "wallet-demo-3".into(),
            address: "0xDemo3cC3C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3".into(),
            label: "experiments".into(),
            chain_id: 137,
            wallet_type: "browser".into(),
            created_at: SEED_NOW_MS - 7 * 86_400_000,
        },
        SeedWallet {
            id: "wallet-demo-4".into(),
            address: "0xDemo4dD4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4".into(),
            label: "archived".into(),
            chain_id: 137,
            wallet_type: "browser".into(),
            created_at: SEED_NOW_MS - 90 * 86_400_000,
        },
    ]
}

fn seed_markets() -> Vec<SeedMarket> {
    // Mix of active, resolved, and expiring markets across 3 categories.
    // end_date is relative to SEED_NOW_MS (in ms).
    let day = 86_400_000;
    let hour = 3_600_000;
    vec![
        // ── Football (4) ────────────────────────────────────────────
        SeedMarket {
            id: "mkt-football-eu-final-2026".into(),
            slug: "champions-league-final-2026-winner".into(),
            question: "Will Real Madrid win the 2026 UEFA Champions League final?".into(),
            description: "Resolves YES if Real Madrid wins the final, NO otherwise.".into(),
            category: "football".into(),
            tags: "uefa,champions-league,real-madrid".into(),
            end_date: SEED_NOW_MS + 14 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("184230.50".into()),
            volume_24h: Some("12340.00".into()),
            created_at: SEED_NOW_MS - 30 * day,
            updated_at: SEED_NOW_MS - 1 * hour,
        },
        SeedMarket {
            id: "mkt-football-premier-top4".into(),
            slug: "premier-league-2025-26-top-4".into(),
            question: "Will Arsenal finish in the Premier League top 4 this season?".into(),
            description: "Resolves YES if Arsenal finish 1st-4th.".into(),
            category: "football".into(),
            tags: "premier-league,arsenal,top-4".into(),
            end_date: SEED_NOW_MS + 60 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("98712.30".into()),
            volume_24h: Some("5400.00".into()),
            created_at: SEED_NOW_MS - 45 * day,
            updated_at: SEED_NOW_MS - 2 * hour,
        },
        SeedMarket {
            id: "mkt-football-la-liga".into(),
            slug: "la-liga-2025-26-winner".into(),
            question: "Who will win La Liga 2025-26?".into(),
            description: "Multi-outcome market.".into(),
            category: "football".into(),
            tags: "la-liga,winner".into(),
            end_date: SEED_NOW_MS + 90 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("156000.00".into()),
            volume_24h: Some("8200.00".into()),
            created_at: SEED_NOW_MS - 50 * day,
            updated_at: SEED_NOW_MS - 3 * hour,
        },
        SeedMarket {
            id: "mkt-football-world-cup".into(),
            slug: "world-cup-2026-winner".into(),
            question: "Who will win the 2026 FIFA World Cup?".into(),
            description: "Multi-outcome market.".into(),
            category: "football".into(),
            tags: "fifa,world-cup,national".into(),
            end_date: SEED_NOW_MS + 200 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("420000.00".into()),
            volume_24h: Some("18750.00".into()),
            created_at: SEED_NOW_MS - 60 * day,
            updated_at: SEED_NOW_MS - 5 * hour,
        },
        // ── CS2 (4) ────────────────────────────────────────────────
        SeedMarket {
            id: "mkt-cs2-blast-finals".into(),
            slug: "blast-world-final-2025-winner".into(),
            question: "Will Team Spirit win the BLAST World Final 2025?".into(),
            description: "Resolves YES if Team Spirit wins.".into(),
            category: "cs2".into(),
            tags: "blast,cs2,team-spirit".into(),
            end_date: SEED_NOW_MS - 2 * day,    // already ended
            active: false,
            resolved: true,
            outcome: Some("YES".into()),
            liquidity: Some("42100.00".into()),
            volume_24h: Some("0.00".into()),
            created_at: SEED_NOW_MS - 30 * day,
            updated_at: SEED_NOW_MS - 2 * day,
        },
        SeedMarket {
            id: "mkt-cs2-major-2026".into(),
            slug: "cs2-major-2026-winner".into(),
            question: "Who will win the next CS2 Major?".into(),
            description: "Multi-outcome.".into(),
            category: "cs2".into(),
            tags: "cs2,major,valve".into(),
            end_date: SEED_NOW_MS + 45 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("78900.00".into()),
            volume_24h: Some("3120.00".into()),
            created_at: SEED_NOW_MS - 14 * day,
            updated_at: SEED_NOW_MS - 4 * hour,
        },
        SeedMarket {
            id: "mkt-cs2-iem-katowice".into(),
            slug: "iem-katowice-2026-winner".into(),
            question: "Will G2 win IEM Katowice 2026?".into(),
            description: "Resolves YES if G2 wins.".into(),
            category: "cs2".into(),
            tags: "cs2,iem,g2".into(),
            end_date: SEED_NOW_MS + 21 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("34000.00".into()),
            volume_24h: Some("1900.00".into()),
            created_at: SEED_NOW_MS - 7 * day,
            updated_at: SEED_NOW_MS - 6 * hour,
        },
        SeedMarket {
            id: "mkt-cs2-blast-spring".into(),
            slug: "blast-spring-2026-finals".into(),
            question: "Will FaZe Clan make the BLAST Spring 2026 finals?".into(),
            description: "Resolves YES if FaZe reach the final match.".into(),
            category: "cs2".into(),
            tags: "cs2,blast,fa-ze".into(),
            end_date: SEED_NOW_MS - 1 * day,    // ended, unresolved for demo
            active: false,
            resolved: true,
            outcome: Some("NO".into()),
            liquidity: Some("22300.00".into()),
            volume_24h: Some("0.00".into()),
            created_at: SEED_NOW_MS - 21 * day,
            updated_at: SEED_NOW_MS - 1 * day,
        },
        // ── Politics (4) ────────────────────────────────────────────
        SeedMarket {
            id: "mkt-politics-2028-pres".into(),
            slug: "us-presidential-2028-winner".into(),
            question: "Who will win the 2028 US presidential election?".into(),
            description: "Multi-outcome market.".into(),
            category: "politics".into(),
            tags: "us,election,2028".into(),
            end_date: SEED_NOW_MS + 365 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("1240000.00".into()),
            volume_24h: Some("84200.00".into()),
            created_at: SEED_NOW_MS - 90 * day,
            updated_at: SEED_NOW_MS - 1 * hour,
        },
        SeedMarket {
            id: "mkt-politics-fed-rate".into(),
            slug: "fed-rate-cut-march-2026".into(),
            question: "Will the Fed cut rates at the March 2026 FOMC meeting?".into(),
            description: "Resolves YES if the federal funds rate is cut by ≥25bp.".into(),
            category: "politics".into(),
            tags: "fed,fomc,rates".into(),
            end_date: SEED_NOW_MS + 30 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("89000.00".into()),
            volume_24h: Some("4200.00".into()),
            created_at: SEED_NOW_MS - 14 * day,
            updated_at: SEED_NOW_MS - 8 * hour,
        },
        SeedMarket {
            id: "mkt-politics-uk-election".into(),
            slug: "uk-general-election-2026".into(),
            question: "Will the UK hold a general election in 2026?".into(),
            description: "Resolves YES if a general election is called and held.".into(),
            category: "politics".into(),
            tags: "uk,election".into(),
            end_date: SEED_NOW_MS + 180 * day,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: Some("34000.00".into()),
            volume_24h: Some("1100.00".into()),
            created_at: SEED_NOW_MS - 30 * day,
            updated_at: SEED_NOW_MS - 12 * hour,
        },
        SeedMarket {
            id: "mkt-politics-g7-summit".into(),
            slug: "g7-summit-2026-joint-statement".into(),
            question: "Will the G7 2026 summit issue a joint statement on AI governance?".into(),
            description: "Resolves YES if the final communiqué mentions AI governance.".into(),
            category: "politics".into(),
            tags: "g7,ai,governance".into(),
            end_date: SEED_NOW_MS - 5 * day,    // ended
            active: false,
            resolved: true,
            outcome: Some("YES".into()),
            liquidity: Some("12300.00".into()),
            volume_24h: Some("0.00".into()),
            created_at: SEED_NOW_MS - 30 * day,
            updated_at: SEED_NOW_MS - 5 * day,
        },
    ]
}

fn seed_signets(markets: &[SeedMarket]) -> Vec<SeedSignal> {
    // 1-2 signals per market, focused on the active ones (more interesting for the UI).
    let day = 86_400_000;
    let mut out = Vec::new();
    let mut next_id = 1000i64;
    for m in markets {
        if !m.active { continue; }
        // 1-2 signals per active market
        let primary = SeedSignal {
            market_id: m.id.clone(),
            computed_at: SEED_NOW_MS - 2 * 3_600_000,
            model_version: "logistic-0.1.0".into(),
            predicted_prob: clamp_prob(0.45 + 0.3 * ((next_id % 5) as f64) / 10.0),
            market_prob: 0.50,
            edge: 0.0, // overwritten below
            confidence: 0.55 + 0.2 * ((next_id % 4) as f64) / 10.0,
            horizon_hours: match m.category.as_str() {
                "football" => 24,
                "cs2" => 12,
                "politics" => 72,
                _ => 24,
            },
            rationale: format!(
                "logistic-0.1.0: category={} edge-vs-market",
                m.category
            ),
        };
        let mut primary = primary;
        primary.edge = primary.predicted_prob - primary.market_prob;
        out.push(primary);
        next_id += 1;
        // Second signal for football/politics markets only
        if matches!(m.category.as_str(), "football" | "politics") {
            let mut secondary = SeedSignal {
                market_id: m.id.clone(),
                computed_at: SEED_NOW_MS - 1 * 3_600_000,
                model_version: "logistic-0.1.0".into(),
                predicted_prob: clamp_prob(0.35 + 0.4 * ((next_id % 7) as f64) / 10.0),
                market_prob: 0.55,
                edge: 0.0,
                confidence: 0.45 + 0.3 * ((next_id % 3) as f64) / 10.0,
                horizon_hours: 48,
                rationale: format!("logistic-0.1.0: re-rank with conservative features"),
            };
            secondary.edge = secondary.predicted_prob - secondary.market_prob;
            out.push(secondary);
            next_id += 1;
        }
        // 5 minutes of unused time elapses
        let _ = day;
    }
    out
}

fn clamp_prob(p: f64) -> f64 { p.max(0.05).min(0.95) }

fn seed_bets(
    wallets: &[SeedWallet],
    markets: &[SeedMarket],
    _signals: &[SeedSignal],
) -> Vec<SeedBet> {
    // 18 bets across the wallets; mix of:
    //   - 10 settled (5 win, 4 loss, 1 cancelled)
    //   - 6 open
    //   - 2 with tx_hash (Mode B signed)
    let day = 86_400_000;
    let hour = 3_600_000;
    let w1 = &wallets[0].id;
    let w2 = &wallets[1].id;
    let w3 = &wallets[2].id;

    let mk = |id: &str, wallet: &str, market: &str, mode: &str, side: &str,
              size: &str, price: f64, shares: &str, placed_at: i64,
              settled_at: Option<i64>, pnl: Option<&str>, status: &str,
              tx: Option<&str>, notes: Option<&str>| SeedBet {
        id: id.into(),
        wallet_id: wallet.into(),
        market_id: market.into(),
        signal_id: None,
        decision_id: None,
        was_llm_assisted: id.contains("llm"),
        mode: mode.into(),
        side: side.into(),
        size: size.into(),
        price,
        shares: shares.into(),
        placed_at,
        settled_at,
        pnl: pnl.map(String::from),
        status: status.into(),
        tx_hash: tx.map(String::from),
        notes: notes.map(String::from),
    };

    vec![
        // ── Settled YES (5) ──────────────────────────────────────
        mk("bet-001", w1, "mkt-cs2-blast-finals", "jump", "yes", "25.00", 0.42, "59.52", SEED_NOW_MS - 7 * day, Some(SEED_NOW_MS - 2 * day), Some("+25.00"), "settled_yes", None, Some("settled on YES outcome")),
        mk("bet-002", w1, "mkt-politics-g7-summit", "signed", "yes", "50.00", 0.61, "81.97", SEED_NOW_MS - 12 * day, Some(SEED_NOW_MS - 5 * day), Some("+30.00"), "settled_yes", Some("0xdemo001"), Some("Mode B signed via keyring")),
        mk("bet-003-llm", w2, "mkt-cs2-blast-spring", "signed", "no", "30.00", 0.58, "51.72", SEED_NOW_MS - 8 * day, Some(SEED_NOW_MS - 1 * day), Some("+30.00"), "settled_yes", Some("0xdemo003"), Some("LLM-assisted: GPT-4o + Claude consensus")),
        mk("bet-004", w1, "mkt-politics-g7-summit", "jump", "yes", "15.00", 0.45, "33.33", SEED_NOW_MS - 14 * day, Some(SEED_NOW_MS - 5 * day), Some("+15.00"), "settled_yes", None, None),
        mk("bet-005", w3, "mkt-cs2-blast-finals", "jump", "yes", "10.00", 0.55, "18.18", SEED_NOW_MS - 6 * day, Some(SEED_NOW_MS - 2 * day), Some("+10.00"), "settled_yes", None, None),
        // ── Settled NO / loss (4) ────────────────────────────────
        mk("bet-006-llm", w2, "mkt-cs2-blast-spring", "signed", "yes", "20.00", 0.55, "36.36", SEED_NOW_MS - 10 * day, Some(SEED_NOW_MS - 1 * day), Some("-20.00"), "settled_no", Some("0xdemo006"), Some("LLM said YES, market said NO")),
        mk("bet-007", w1, "mkt-cs2-blast-spring", "jump", "yes", "12.00", 0.62, "19.35", SEED_NOW_MS - 9 * day, Some(SEED_NOW_MS - 1 * day), Some("-12.00"), "settled_no", None, None),
        mk("bet-008", w3, "mkt-cs2-blast-finals", "jump", "no", "8.00", 0.48, "16.67", SEED_NOW_MS - 5 * day, Some(SEED_NOW_MS - 2 * day), Some("-8.00"), "settled_no", None, None),
        mk("bet-009", w1, "mkt-politics-g7-summit", "jump", "no", "5.00", 0.30, "16.67", SEED_NOW_MS - 13 * day, Some(SEED_NOW_MS - 5 * day), Some("-5.00"), "settled_no", None, Some("hedge")),
        // ── Cancelled (1) ────────────────────────────────────────
        mk("bet-010", w1, "mkt-football-premier-top4", "jump", "yes", "10.00", 0.50, "20.00", SEED_NOW_MS - 3 * day, None, None, "cancelled", None, Some("user cancelled from UI")),
        // ── Open (6) ─────────────────────────────────────────────
        mk("bet-011", w2, "mkt-football-eu-final-2026", "signed", "yes", "40.00", 0.38, "105.26", SEED_NOW_MS - 1 * day, None, None, "open", Some("0xdemo011"), Some("Mode B open, watching finals")),
        mk("bet-012-llm", w2, "mkt-cs2-major-2026", "signed", "yes", "25.00", 0.32, "78.13", SEED_NOW_MS - 12 * hour, None, None, "open", Some("0xdemo012"), Some("LLM consensus: place at <0.35")),
        mk("bet-013", w1, "mkt-politics-fed-rate", "jump", "yes", "60.00", 0.41, "146.34", SEED_NOW_MS - 8 * hour, None, None, "open", None, None),
        mk("bet-014", w3, "mkt-football-la-liga", "jump", "no", "15.00", 0.55, "27.27", SEED_NOW_MS - 4 * hour, None, None, "open", None, None),
        mk("bet-015-llm", w2, "mkt-cs2-iem-katowice", "signed", "no", "20.00", 0.42, "47.62", SEED_NOW_MS - 2 * hour, None, None, "open", Some("0xdemo015"), Some("LLM majority: NO")),
        mk("bet-016", w1, "mkt-politics-2028-pres", "jump", "yes", "100.00", 0.18, "555.56", SEED_NOW_MS - 30 * 60_000, None, None, "open", None, Some("longshot, low conviction")),
    ]
    .into_iter()
    .map(|b| {
        // `_markets` and `_signals` are intentionally unused here; the
        // array is just there so the caller can reference ids without
        // us hardcoding them above. Silence the warning.
        let _ = (markets, _signals);
        b
    })
    .collect()
}

fn seed_copy_targets() -> Vec<SeedCopyTarget> {
    let day = 86_400_000;
    vec![
        SeedCopyTarget {
            id: "copy-tgt-trader-alpha".into(),
            address: "0xAlphaA1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1".into(),
            label: "trader-alpha (sports sharp)".into(),
            enabled: true,
            allocation_cap: "200.00".into(),
            min_edge: 0.05,
            created_at: SEED_NOW_MS - 30 * day,
        },
        SeedCopyTarget {
            id: "copy-tgt-quant-fund".into(),
            address: "0xQuantQQ1Q1Q1Q1Q1Q1Q1Q1Q1Q1Q1Q1Q1Q1Q1Q1Q1Q".into(),
            label: "quant-fund (mid-size)".into(),
            enabled: true,
            allocation_cap: "500.00".into(),
            min_edge: 0.03,
            created_at: SEED_NOW_MS - 21 * day,
        },
        SeedCopyTarget {
            id: "copy-tgt-inst-desk".into(),
            address: "0xInstI1I1I1I1I1I1I1I1I1I1I1I1I1I1I1I1I1I1".into(),
            label: "institutional-desk (whale)".into(),
            enabled: false,    // disabled — interesting UI state
            allocation_cap: "1000.00".into(),
            min_edge: 0.02,
            created_at: SEED_NOW_MS - 60 * day,
        },
    ]
}

fn seed_copy_events(
    targets: &[SeedCopyTarget],
    markets: &[SeedMarket],
    bets: &[SeedBet],
) -> Vec<SeedCopyEvent> {
    let day = 86_400_000;
    let mut out = Vec::new();
    let active_markets: Vec<&SeedMarket> = markets.iter().collect();
    let settled_bets: Vec<&SeedBet> = bets
        .iter()
        .filter(|b| b.status == "settled_yes" || b.status == "settled_no")
        .collect();

    for (i, t) in targets.iter().enumerate() {
        if !t.enabled && i == 2 { continue; }  // skip the disabled one for events
        // 2 events per active target
        for j in 0..2i64 {
            let market = active_markets[((i as i64) * 2 + j) as usize % active_markets.len()];
            let matched = if j == 0 && !settled_bets.is_empty() {
                Some(settled_bets[(((i as i64) * 2 + j) as usize) % settled_bets.len()].id.clone())
            } else {
                None
            };
            out.push(SeedCopyEvent {
                target_id: t.id.clone(),
                market_id: market.id.clone(),
                detected_at: SEED_NOW_MS - (5 - i as i64) * day - j * 3_600_000,
                side: if j == 0 { "yes".into() } else { "no".into() },
                size: format!("{}.00", 15 + (i as i64) * 5 + j * 3),
                price: 0.4 + 0.05 * (i as f64 + j as f64),
                tx_hash: format!("0xCopyTx{:04x}{:04x}", i, j),
                matched_bet_id: matched,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_bundle_has_realistic_counts() {
        let b = SeedBundle::demo();
        assert!(b.wallets.len() >= 3, "need at least 3 wallets, got {}", b.wallets.len());
        assert!(b.markets.len() >= 10, "need at least 10 markets, got {}", b.markets.len());
        assert!(b.signals.len() >= 10, "need at least 10 signals, got {}", b.signals.len());
        assert!(b.bets.len() >= 15, "need at least 15 bets, got {}", b.bets.len());
        assert!(b.copy_targets.len() >= 2, "need at least 2 copy targets, got {}", b.copy_targets.len());
        assert!(b.copy_events.len() >= 4, "need at least 4 copy events, got {}", b.copy_events.len());
        assert!(b.total_rows() >= 50);
    }

    #[test]
    fn all_bets_reference_real_wallets_and_markets() {
        let b = SeedBundle::demo();
        let wallet_ids: std::collections::HashSet<&str> = b.wallets.iter().map(|w| w.id.as_str()).collect();
        let market_ids: std::collections::HashSet<&str> = b.markets.iter().map(|m| m.id.as_str()).collect();
        for bet in &b.bets {
            assert!(wallet_ids.contains(bet.wallet_id.as_str()), "bet {} refs unknown wallet {}", bet.id, bet.wallet_id);
            assert!(market_ids.contains(bet.market_id.as_str()), "bet {} refs unknown market {}", bet.id, bet.market_id);
        }
    }

    #[test]
    fn all_signals_reference_real_markets() {
        let b = SeedBundle::demo();
        let market_ids: std::collections::HashSet<&str> = b.markets.iter().map(|m| m.id.as_str()).collect();
        for s in &b.signals {
            assert!(market_ids.contains(s.market_id.as_str()), "signal refs unknown market {}", s.market_id);
        }
    }

    #[test]
    fn all_copy_events_reference_real_targets_and_markets() {
        let b = SeedBundle::demo();
        let target_ids: std::collections::HashSet<&str> = b.copy_targets.iter().map(|t| t.id.as_str()).collect();
        let market_ids: std::collections::HashSet<&str> = b.markets.iter().map(|m| m.id.as_str()).collect();
        for e in &b.copy_events {
            assert!(target_ids.contains(e.target_id.as_str()), "event refs unknown target {}", e.target_id);
            assert!(market_ids.contains(e.market_id.as_str()), "event refs unknown market {}", e.market_id);
        }
    }

    #[test]
    fn signal_edges_are_predicted_minus_market() {
        let b = SeedBundle::demo();
        for s in &b.signals {
            let expected = s.predicted_prob - s.market_prob;
            assert!(
                (s.edge - expected).abs() < 1e-9,
                "signal edge {} != predicted - market ({} - {})",
                s.edge, s.predicted_prob, s.market_prob
            );
        }
    }

    #[test]
    fn seed_now_is_2026_jan_15() {
        // Pinning the reference time so screenshots and tests stay reproducible.
        assert_eq!(SEED_NOW_MS, 1_768_456_800_000);
    }

    #[test]
    fn ids_are_stable_across_runs() {
        // Run the seeder twice and check the IDs are identical.
        let a = SeedBundle::demo();
        let b = SeedBundle::demo();
        let a_ids: Vec<&str> = a.markets.iter().map(|m| m.id.as_str()).collect();
        let b_ids: Vec<&str> = b.markets.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(a_ids, b_ids);
    }

    #[test]
    fn all_active_markets_have_signals() {
        let b = SeedBundle::demo();
        let active_markets: Vec<&str> = b.markets.iter().filter(|m| m.active).map(|m| m.id.as_str()).collect();
        let signal_markets: std::collections::HashSet<&str> = b.signals.iter().map(|s| s.market_id.as_str()).collect();
        for m in active_markets {
            assert!(signal_markets.contains(m), "active market {} has no signals", m);
        }
    }
}
