//! L2 — Demo data seeder (M13+).
//!
//! IPCs:
//! - `seed_demo_data(force)`     — apply the canonical demo bundle
//! - `is_seeded()`               — report whether the DB already has data
//!
//! The auto-seed on first launch happens inside `init_pool` (see
//! `infra::db::seed`); this IPC is for explicit re-seed (Settings →
//! "Reset demo data" or for a dev helper).

use crate::infra::db;
use crate::infra::error::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedStatus {
    pub seeded: bool,
    pub last_seed_rows: i64,
    pub last_seed_at_ms: i64,
}

#[derive(Debug, Deserialize)]
pub struct SeedArgs {
    /// Re-apply even if already seeded (resets the demo bundle).
    pub force: Option<bool>,
}

/// Apply the demo bundle. Idempotent unless `force=true`.
#[tauri::command]
pub async fn seed_demo_data(
    state: State<'_, AppState>,
    args: SeedArgs,
) -> AppResult<usize> {
    let force = args.force.unwrap_or(false);
    let inserted = db::apply_seed(&state.db, force).await?;
    Ok(inserted)
}

/// Returns whether the DB has demo data already.
#[tauri::command]
pub async fn is_seeded(state: State<'_, AppState>) -> AppResult<bool> {
    db::is_seeded(&state.db).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_args_default_force_is_false() {
        // The default is false (idempotent); force must be explicit.
        let a = SeedArgs { force: None };
        assert_eq!(a.force.unwrap_or(false), false);
    }
}
