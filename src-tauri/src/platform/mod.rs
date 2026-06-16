//! L5 — Platform Layer.
//!
//! This layer is the **only** place in polyrocket that talks to the host
//! operating system directly. Everything else (L1-L4) goes through here.
//!
//! Modules
//! -------
//! - [`keyring`]  — macOS Keychain / Windows Credential Manager / Linux
//!   Secret Service. The `keyring` crate handles cross-platform
//!   abstraction, so the per-OS code is a single factory + service name.
//! - [`env`]      — read `POLYROCKET_*` env vars + dev-mode `.env` sync.
//! - [`paths`]    — resolve app data dir, db path, log path.
//!
//! Layer rules
//! -----------
//! - L5 must NOT depend on L1, L2, L3, or L4 (would create cycles).
//! - L5 modules MAY import each other (env uses keyring, paths is leaf).
//! - All secrets leave / enter the process **only** via `keyring::*`.
//!
//! See `docs/overview.md` §2.1 for the full module map.

pub mod env;
pub mod keyring;
pub mod paths;

// Re-export the high-level API surface so callers can `use crate::platform::*;`
// without going three levels deep.
pub use env::{maybe_load_dev_env, parse_env_file};
pub use keyring::{
    delete_key, delete_provider_keys, get_key, has_key, set_key,
    llm_alias, pm_api_alias, pm_passphrase_alias, pm_secret_alias, wallet_alias,
};
pub use paths::{app_data_dir, db_path, log_dir, sqlite_url};
