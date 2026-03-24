use std::collections::BTreeMap;

use draton_stdlib::json;
use draton_stdlib::JsonValue;
use pretty_assertions::assert_eq;

#[test]
fn parse_stringify_and_pretty_roundtrip() {
    let value = json::parse(r#"{"name":"draton","ok":true,"nums":[1,2]}"#).expect("parse");
    let compact = json::stringify(value.clone());
    let reparsed = json::parse(&compact).expect("reparse");
    let pretty = json::pretty(value.clone());

    assert_eq!(value, reparsed);
    assert!(pretty.contains('\n'));
}

#[test]
fn invalid_json_reports_position() {
    let error = json::parse("{ bad json ]").expect_err("json should fail");
    assert!(error.line() >= 1);
    assert!(error.col() >= 1);
    assert!(!error.message().is_empty());
}

#[test]
fn response_json_uses_same_parser() {
    let mut headers = BTreeMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());
    let response = draton_stdlib::Response::new(200, r#"{"ok":true}"#, headers);

    assert_eq!(
        response.json().expect("response json"),
        JsonValue::Object(BTreeMap::from([("ok".to_string(), JsonValue::Bool(true))]))
    );
}
