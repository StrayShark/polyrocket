//! polyrocket — Tauri main entry
//!
//! Spec: polyradar-blueprint-v2-client.md §3 (IPC commands)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    polyrocket_lib::run()
}