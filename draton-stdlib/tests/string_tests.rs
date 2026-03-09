use draton_stdlib::string;
use pretty_assertions::assert_eq;

#[test]
fn string_transforms_cover_core_api() {
    assert_eq!(string::upper("draton"), "DRATON");
    assert_eq!(string::lower("DrAtOn"), "draton");
    assert_eq!(string::trim("  hi \n"), "hi");
    assert_eq!(string::trim_start("  hi"), "hi");
    assert_eq!(string::trim_end("hi  "), "hi");
    assert_eq!(string::split("a,b,c", ","), vec!["a", "b", "c"]);
    assert_eq!(
        string::join(&["a".to_string(), "b".to_string(), "c".to_string()], "-"),
        "a-b-c"
    );
    assert!(string::contains("compiler", "pile"));
    assert!(string::starts_with("compiler", "com"));
    assert!(string::ends_with("compiler", "ler"));
    assert_eq!(string::replace("banana", "na", "XX"), "baXXna");
    assert_eq!(string::replace_all("banana", "na", "XX"), "baXXXX");
    assert_eq!(string::len("ábc"), 3);
    assert_eq!(string::chars("abc"), vec!['a', 'b', 'c']);
    assert_eq!(string::repeat("xo", 3), "xoxoxo");
    assert_eq!(string::index_of("compiler", "pile"), Some(3));
    assert_eq!(string::slice("compiler", 0, 4), "comp");
}

#[test]
fn string_parse_errors_are_reported() {
    let int_error = string::to_int("nan").expect_err("int parse should fail");
    let float_error = string::to_float("nanf").expect_err("float parse should fail");

    assert!(!int_error.message().is_empty());
    assert!(!float_error.message().is_empty());
}
