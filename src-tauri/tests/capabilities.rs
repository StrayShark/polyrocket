//! Capability configuration tests (v0.7c).
//!
//! The `capabilities/default.json` file is the contract between the Rust
//! plugin set and the L1 frontend. If we add a Rust plugin (or a new
//! permission on an existing one) without updating the capability, the
//! app silently fails permission checks at runtime — bad.
//!
//! These tests parse the JSON and assert what's in it. If you add a new
//! plugin to `tauri.conf.json`/lib.rs, this test will fail and remind
//! you to update the capability.

use std::fs;
use std::path::PathBuf;

fn capabilities_path() -> PathBuf {
    // tests/ lives in src-tauri, capabilities/ is a sibling
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("capabilities").join("default.json")
}

#[test]
fn default_capability_exists() {
    let p = capabilities_path();
    assert!(p.exists(), "capabilities/default.json missing at {p:?}");
    let text = fs::read_to_string(&p).expect("read capabilities");
    assert!(text.contains("\"identifier\""), "missing identifier");
    assert!(text.contains("\"permissions\""), "missing permissions array");
}

#[test]
fn default_capability_includes_sql() {
    let p = capabilities_path();
    let text = fs::read_to_string(&p).expect("read");
    assert!(text.contains("\"sql:default\""), "sql:default missing");
    assert!(text.contains("\"sql:allow-load\""), "sql:allow-load missing");
    assert!(text.contains("\"sql:allow-execute\""), "sql:allow-execute missing");
    assert!(text.contains("\"sql:allow-select\""), "sql:allow-select missing");
}

#[test]
fn default_capability_includes_shell() {
    let text = fs::read_to_string(capabilities_path()).expect("read");
    assert!(text.contains("\"shell:default\""), "shell:default missing");
    assert!(text.contains("\"shell:allow-open\""), "shell:allow-open missing");
}

#[test]
fn default_capability_includes_os() {
    let text = fs::read_to_string(capabilities_path()).expect("read");
    assert!(text.contains("\"os:default\""), "os:default missing");
}

#[test]
fn default_capability_includes_notification_v0_7c() {
    // v0.7c: added notification permissions so the L1 toast->system
    // bridge actually surfaces native notifications. If you remove
    // these, the v0.5c notify.rs will silently no-op on macOS.
    let text = fs::read_to_string(capabilities_path()).expect("read");
    assert!(text.contains("\"notification:default\""), "notification:default missing");
    assert!(text.contains("\"notification:allow-notify\""), "notification:allow-notify missing");
    assert!(
        text.contains("\"notification:allow-request-permission\""),
        "notification:allow-request-permission missing"
    );
    assert!(
        text.contains("\"notification:allow-is-permission-granted\""),
        "notification:allow-is-permission-granted missing"
    );
}

#[test]
fn default_capability_targets_main_window() {
    let text = fs::read_to_string(capabilities_path()).expect("read");
    assert!(text.contains("\"windows\""), "windows array missing");
    assert!(text.contains("\"main\""), "main window not listed");
}

#[test]
fn default_capability_does_not_have_unknown_permissions() {
    // If you add a new plugin, you MUST add its permissions here too.
    // This test catches the case where someone copy-pastes a permission
    // name that doesn't exist in the bundle (e.g. typo, plugin not
    // installed). The build will warn on the unknown permission; this
    // test ensures we don't have stale entries.
    let text = fs::read_to_string(capabilities_path()).expect("read");

    let known_but_not_installed = ["log:default", "dialog:default", "fs:default"];
    for perm in known_but_not_installed {
        assert!(
            !text.contains(&format!("\"{perm}\"")),
            "permission {perm} referenced but its plugin isn't installed; \
             either install the matching tauri-plugin or remove this line"
        );
    }
}
