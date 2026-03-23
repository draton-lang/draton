use std::collections::BTreeMap;

use draton_stdlib::{crypto, modules, net, os, test_modules, time, JsonValue};
use pretty_assertions::assert_eq;

#[test]
fn bundled_modules_and_dt_tests_are_available() {
    let module_names = modules()
        .iter()
        .map(|module| module.name)
        .collect::<Vec<_>>();
    let test_names = test_modules()
        .iter()
        .map(|module| module.name)
        .collect::<Vec<_>>();

    assert_eq!(
        module_names,
        vec![
            "fs",
            "net",
            "io",
            "string",
            "os",
            "time",
            "json",
            "math",
            "collections",
            "crypto",
            "test",
        ]
    );
    assert_eq!(
        test_names,
        vec!["fs_tests", "string_tests", "json_tests", "math_tests"]
    );
    assert!(modules()
        .iter()
        .all(|module| !module.source.trim().is_empty()));
    assert!(test_modules()
        .iter()
        .all(|module| !module.source.trim().is_empty()));
}

#[test]
fn os_time_and_crypto_helpers_work() {
    os::set_env("DRATON_STDLIB_TEST_KEY", "value");
    assert_eq!(
        os::env_var("DRATON_STDLIB_TEST_KEY"),
        Some("value".to_string())
    );
    assert!(!os::platform().is_empty());
    assert!(!os::arch().is_empty());
    assert!(os::pid() > 0);
    assert!(!os::hostname().is_empty());

    let before = time::now();
    time::sleep(5);
    let elapsed = time::since(before);
    assert!(elapsed.ms() >= 0);
    assert!(!before.format("%Y").is_empty());

    assert_eq!(
        crypto::sha256("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        crypto::sha512("abc"),
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
    );
    assert_eq!(crypto::md5("abc"), "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(crypto::uuid().len(), 36);
    assert_eq!(crypto::random_bytes(4).len(), 4);
    let sample = crypto::random_int(3, 7);
    assert!((3..=7).contains(&sample));
}

#[test]
fn response_and_net_errors_are_exposed() {
    let response = draton_stdlib::Response::new(
        201,
        r#"{"value":1}"#,
        BTreeMap::from([("content-type".to_string(), "application/json".to_string())]),
    );
    assert_eq!(response.status(), 201);
    assert_eq!(response.text(), r#"{"value":1}"#);
    assert_eq!(
        response.json().expect("response json"),
        JsonValue::Object(BTreeMap::from([("value".to_string(), JsonValue::Int(1))]))
    );
    let headers = response.headers();
    assert_eq!(
        headers.get("content-type"),
        Some(&"application/json".to_string())
    );

    assert!(net::get("::bad-url::").is_err());
}
