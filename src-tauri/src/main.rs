//! polyrocket — Tauri 主入口
//!
//! 规范：polyradar-blueprint-v2-client.md §3（IPC 命令）

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    polyrocket_lib::run()
}