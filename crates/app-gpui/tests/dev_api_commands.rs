//! Integration tests for the dev-api command/reply types.
//!
//! The dev-api module is excluded from the library's test build, so we
//! include the standalone source file directly to exercise its pure logic.

#![cfg(feature = "dev-api")]
// The included source defines `DevCommand` fields that are not used in this
// test; suppress dead-code warnings for the test-only inclusion.
#![allow(dead_code)]

#[path = "../app/dev_api/commands.rs"]
mod commands;

use commands::{DevQueryReply, DevReply};

#[test]
fn dev_reply_ok_to_json() {
    assert_eq!(DevReply::ok().to_json(), r#"{"ok":true}"#);
}

#[test]
fn dev_reply_err_to_json() {
    let json = DevReply::err("bad path").to_json();
    assert!(json.contains("\"ok\":false"));
    assert!(json.contains("bad path"));
}

#[test]
fn dev_query_reply_ok_to_json() {
    let json = DevQueryReply::ok(serde_json::json!(42)).to_json();
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("\"value\":42"));
}

#[test]
fn dev_query_reply_err_to_json() {
    let json = DevQueryReply::err("missing path").to_json();
    assert!(json.contains("\"ok\":false"));
    assert!(json.contains("missing path"));
}
