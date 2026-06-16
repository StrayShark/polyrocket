//! L4 — Shared sidecar state (singleton across Tauri commands).
//!
//! Held as a Tauri-managed state so any `#[tauri::command]` can
//! access the running sidecar process via `State<'_, SidecarState>`.

pub use crate::commands::sidecar::{SidecarState, SidecarStatus};
