use super::misc::DEFAULT_DEV_API_URL;
use super::misc::expand_env_vars_with;
use super::misc::focus_action_name;
use super::misc::resolve_base_url;
use super::misc::strip_comment;
use super::misc::urlencode;
use super::parse::parse_compare;
use super::parse::parse_dev_response_body;
use super::parse::parse_duration;
use serde_json::Value;
use std::time::Duration;

use serde_json::json;

#[test]
fn comment_stripping() {
    assert_eq!(strip_comment("foo  # bar").trim(), "foo");
    assert_eq!(strip_comment("# only").trim(), "");
    assert_eq!(strip_comment("plain").trim(), "plain");
    assert_eq!(
        strip_comment(r#"assert title == "issue #42" # trailing"#).trim(),
        r#"assert title == "issue #42""#
    );
}

#[test]
fn duration_suffixes() {
    assert_eq!(parse_duration("250ms").unwrap(), Duration::from_millis(250));
    assert_eq!(parse_duration("2s").unwrap(), Duration::from_secs(2));
    assert_eq!(parse_duration("0.5s").unwrap(), Duration::from_millis(500));
    assert_eq!(parse_duration("1m").unwrap(), Duration::from_secs(60));
    assert_eq!(parse_duration("3").unwrap(), Duration::from_secs(3));
    assert_eq!(
        parse_duration("1.5ms").unwrap(),
        Duration::from_micros(1500)
    );
}

#[test]
fn compare_bool_match() {
    let cmp = parse_compare("playback.is_playing == true").unwrap();
    assert!(cmp.matches(&json!(true)));
    assert!(!cmp.matches(&json!(false)));
    assert!(!cmp.matches(&json!("true")));
}

#[test]
fn compare_number_with_tolerance() {
    let cmp = parse_compare("playback.volume == 0.85 tolerance=0.01").unwrap();
    assert!(cmp.matches(&json!(0.851)));
    assert!(cmp.matches(&json!(0.845)));
    assert!(!cmp.matches(&json!(0.9)));
}

#[test]
fn compare_number_relative_epsilon_without_tolerance() {
    let cmp = parse_compare("playback.volume == 0.3").unwrap();
    assert!(cmp.matches(&json!(0.1f64 + 0.2f64)));
}

#[test]
fn compare_number_ordering() {
    assert!(
        parse_compare("roomeq.filter_count > 0")
            .unwrap()
            .matches(&json!(8))
    );
    assert!(
        parse_compare("roomeq.filter_count >= 8")
            .unwrap()
            .matches(&json!(8))
    );
    assert!(
        parse_compare("roomeq.average_post_score < 35")
            .unwrap()
            .matches(&json!(26.5))
    );
    assert!(
        parse_compare("roomeq.average_post_score <= 26.5")
            .unwrap()
            .matches(&json!(26.5))
    );
    assert!(
        !parse_compare("roomeq.average_post_score < 20")
            .unwrap()
            .matches(&json!(26.5))
    );
}

#[test]
fn compare_not_equal() {
    assert!(
        parse_compare("roomeq.error != null")
            .unwrap()
            .matches(&json!("boom"))
    );
    assert!(
        !parse_compare("roomeq.error != null")
            .unwrap()
            .matches(&Value::Null)
    );
    assert!(
        parse_compare("screen.focused != Library")
            .unwrap()
            .matches(&json!("RoomEq"))
    );
}

#[test]
fn compare_string_quoted_and_bare() {
    let cmp = parse_compare(r#"screen.focused == "Library""#).unwrap();
    assert!(cmp.matches(&json!("Library")));
    assert!(!cmp.matches(&json!("Queue")));

    let cmp_bare = parse_compare("screen.focused == Queue").unwrap();
    assert!(cmp_bare.matches(&json!("Queue")));
}

#[test]
fn compare_timeout_clause() {
    let cmp = parse_compare("queue.length == 3 timeout=500ms").unwrap();
    assert_eq!(cmp.timeout, Some(Duration::from_millis(500)));
    assert!(cmp.matches(&json!(3)));
}

#[test]
fn compare_keeps_clause_like_text_inside_string_literal() {
    let cmp = parse_compare(r#"screen.focused == "tolerance=high" timeout=500ms"#).unwrap();
    assert_eq!(cmp.expected_text, r#""tolerance=high""#);
    assert_eq!(cmp.timeout, Some(Duration::from_millis(500)));
    assert!(cmp.matches(&json!("tolerance=high")));
}

#[test]
fn compare_rejects_unbalanced_string_quotes() {
    assert!(parse_compare(r#"screen.focused == "Library"#).is_err());
    assert!(parse_compare(r#"screen.focused == Library""#).is_err());
}

#[test]
fn focus_action_name_validates_names() {
    assert_eq!(focus_action_name("room_eq").unwrap(), "SwitchToRoomEq");
    assert_eq!(
        focus_action_name("headphone-eq").unwrap(),
        "SwitchToHeadphoneEq"
    );
    assert!(focus_action_name("2nd_screen").is_err());
    assert!(focus_action_name("room/eq").is_err());
}

#[test]
fn base_url_prefers_cli_then_env_port_then_default() {
    assert_eq!(
        resolve_base_url(Some("http://127.0.0.1:9999"), Some("8888")).unwrap(),
        "http://127.0.0.1:9999"
    );
    assert_eq!(
        resolve_base_url(None, Some("8888")).unwrap(),
        "http://127.0.0.1:8888"
    );
    assert_eq!(resolve_base_url(None, None).unwrap(), DEFAULT_DEV_API_URL);
    assert!(resolve_base_url(None, Some("nope")).is_err());
}

#[test]
fn query_error_includes_plain_text_response_body() {
    let err = parse_dev_response_body(
        reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        "plain panic details",
        "query `playback.volume`",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("query `playback.volume` failed"));
    assert!(err.contains("500 Internal Server Error"));
    assert!(err.contains("plain panic details"));
}

#[test]
fn action_error_includes_json_error_message() {
    let err = parse_dev_response_body(
        reqwest::StatusCode::BAD_REQUEST,
        r#"{"ok":false,"error":"unknown action"}"#,
        "action `Nope`",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("action `Nope` failed"));
    assert!(err.contains("400 Bad Request"));
    assert!(err.contains("unknown action"));
}

#[test]
fn urlencode_safe_chars() {
    assert_eq!(urlencode("playback.volume"), "playback.volume");
    assert_eq!(urlencode("a b"), "a%20b");
    assert_eq!(urlencode("a&b"), "a%26b");
}

#[test]
fn env_var_expansion() {
    let mut vars = std::collections::HashMap::new();
    vars.insert("SOTF_TEST_X".to_string(), "/tmp/qa".to_string());
    let result = expand_env_vars_with("plugin_chain_save $SOTF_TEST_X/gain.json", |name| {
        vars.get(name)
            .cloned()
            .ok_or(std::env::VarError::NotPresent)
    });
    assert_eq!(result, "plugin_chain_save /tmp/qa/gain.json");
}
