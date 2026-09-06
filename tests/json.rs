//! Integration tests for `json`, anchored to RFC 8259 and the failure
//! classes catalogued by JSONTestSuite: a table of valid inputs with their
//! exact compact serialization, a table of inputs that must be rejected,
//! round-trip stability over the whole valid table, number-precision checks,
//! pretty-print goldens, and line/column error positions.

use json::{parse, Number, Value};

/// (input, expected compact serialization).
const VALID: &[(&str, &str)] = &[
    // literals
    ("null", "null"),
    ("true", "true"),
    ("false", "false"),
    // integers
    ("0", "0"),
    ("-0", "0"),
    ("42", "42"),
    ("-7", "-7"),
    ("9223372036854775807", "9223372036854775807"),
    ("-9223372036854775808", "-9223372036854775808"),
    ("9007199254740993", "9007199254740993"), // 2^53 + 1, exact as i64
    // floats: shortest round-trip formatting, ".0" kept on integral floats
    ("3.14", "3.14"),
    ("0.5", "0.5"),
    ("-2.5e2", "-250.0"),
    ("1e2", "100.0"),
    ("1E-2", "0.01"),
    ("1e0", "1.0"),
    ("-0.0", "-0.0"),
    ("1e-3", "0.001"),
    // strings
    ("\"\"", "\"\""),
    ("\"abc\"", "\"abc\""),
    ("\"a\\u0041b\"", "\"aAb\""),
    (
        "\"\\\"\\\\\\/\\b\\f\\n\\r\\t\"",
        "\"\\\"\\\\/\\b\\f\\n\\r\\t\"",
    ),
    ("\"\\u00e9\"", "\"\u{e9}\""),
    ("\"h\u{e9}llo\"", "\"h\u{e9}llo\""),
    ("\"\\ud834\\udd1e\"", "\"\u{1D11E}\""),
    ("\"\\uD83D\\uDE00\"", "\"\u{1F600}\""),
    ("\"\\u0000\"", "\"\\u0000\""),
    ("\"\\u001f\"", "\"\\u001f\""),
    // arrays
    ("[]", "[]"),
    ("[ ]", "[]"),
    ("[1,2,3]", "[1,2,3]"),
    ("[ 1 , 2 ]", "[1,2]"),
    ("[[[]]]", "[[[]]]"),
    ("[null,true,\"x\",1.5]", "[null,true,\"x\",1.5]"),
    // objects: order preserved, duplicates preserved
    ("{}", "{}"),
    ("{ }", "{}"),
    ("{\"a\":1}", "{\"a\":1}"),
    ("{\"a\":{\"b\":[]}}", "{\"a\":{\"b\":[]}}"),
    ("{\"b\":1,\"a\":2}", "{\"b\":1,\"a\":2}"),
    ("{\"a\":1,\"a\":2}", "{\"a\":1,\"a\":2}"),
    // whitespace and BOM
    (" \t\r\n1 \t\r\n", "1"),
    ("\u{feff}{}", "{}"),
];

/// Inputs that a strict RFC 8259 parser must reject.
const INVALID: &[&str] = &[
    // structure
    "",
    "   ",
    "{",
    "[",
    "\"",
    "{]",
    "[}",
    "[1,]",
    "[,1]",
    "{\"a\":1,}",
    "{\"a\":}",
    "{\"a\"}",
    "{\"a\" 1}",
    "{a:1}",
    "{'a':1}",
    "'x'",
    "[1 2]",
    "[\"a\": 1]",
    "{\"a\":1 \"b\":2}",
    "[[[",
    "[1]]",
    // trailing content
    "1 2",
    "null null",
    "{} {}",
    "nulll",
    // numbers
    "01",
    "-01",
    "+1",
    "1.",
    ".5",
    "1e",
    "1e+",
    "-",
    "--1",
    "1.2.3",
    "Infinity",
    "-Infinity",
    "NaN",
    "nan",
    "1e999",
    "-1e999",
    // literals
    "True",
    "tru",
    "falze",
    "nul",
    "None",
    // comments
    "[1] // x",
    "/* */ 1",
    // strings
    "\"abc",
    "\"\\x\"",
    "\"\\u12\"",
    "\"\\u12g4\"",
    "\"\\ud800\"",
    "\"\\ud800\\u0041\"",
    "\"\\udc00\"",
    "\"a\tb\"",
    "\"a\nb\"",
];

#[test]
fn valid_inputs_serialize_byte_exact() {
    for (input, expected) in VALID {
        let v = parse(input).unwrap_or_else(|e| panic!("{input:?} failed to parse: {e}"));
        assert_eq!(v.to_string(), *expected, "input: {input:?}");
    }
}

#[test]
fn round_trip_is_stable() {
    for (input, expected) in VALID {
        let v = parse(input).unwrap();
        let again = parse(expected).unwrap_or_else(|e| panic!("{expected:?} reparse: {e}"));
        assert_eq!(v, again, "value changed across round trip for {input:?}");
        assert_eq!(again.to_string(), *expected, "serialization not a fixpoint");
    }
}

#[test]
fn invalid_inputs_are_rejected() {
    for input in INVALID {
        assert!(parse(input).is_err(), "{input:?} should have been rejected");
    }
}

#[test]
fn integer_precision_is_exact() {
    assert_eq!(
        parse("9223372036854775807").unwrap().as_i64(),
        Some(i64::MAX)
    );
    assert_eq!(
        parse("-9223372036854775808").unwrap().as_i64(),
        Some(i64::MIN)
    );
    assert_eq!(
        parse("9007199254740993").unwrap().as_i64(),
        Some(9007199254740993)
    );
}

#[test]
fn oversized_integers_fall_back_to_float() {
    let v = parse("9223372036854775808").unwrap(); // i64::MAX + 1
    assert_eq!(v.as_i64(), None);
    assert_eq!(v.as_f64(), Some(9.223372036854776e18));
}

#[test]
fn number_accessors() {
    assert_eq!(parse("2").unwrap().as_f64(), Some(2.0));
    assert_eq!(parse("2.5").unwrap().as_f64(), Some(2.5));
    assert_eq!(parse("2.5").unwrap().as_i64(), None);
    assert_eq!(parse("2").unwrap(), Value::Number(Number::Int(2)));
}

#[test]
fn duplicate_keys_last_wins_on_get() {
    let v = parse("{\"a\":1,\"a\":2}").unwrap();
    assert_eq!(v.get("a").and_then(Value::as_i64), Some(2));
    assert_eq!(v.as_object().unwrap().len(), 2);
}

#[test]
fn accessors_on_wrong_types_return_none() {
    let v = parse("[1]").unwrap();
    assert_eq!(v.get("a"), None);
    assert_eq!(v.as_str(), None);
    assert_eq!(v.as_bool(), None);
    assert_eq!(v.as_object(), None);
    assert_eq!(parse("{}").unwrap().as_array(), None);
}

#[test]
fn pretty_golden() {
    let v = parse(r#"{"name":"ada","tags":[1,2],"e":{},"ok":true}"#).unwrap();
    let expected = "{\n  \"name\": \"ada\",\n  \"tags\": [\n    1,\n    2\n  ],\n  \"e\": {},\n  \"ok\": true\n}";
    assert_eq!(v.pretty(2), expected);
}

#[test]
fn pretty_scalar_and_empty_containers_stay_inline() {
    assert_eq!(parse("1").unwrap().pretty(2), "1");
    assert_eq!(parse("[]").unwrap().pretty(2), "[]");
    assert_eq!(parse("{}").unwrap().pretty(2), "{}");
}

#[test]
fn error_positions_are_line_and_column_precise() {
    let e = parse("{\n  \"a\": x\n}").unwrap_err();
    assert_eq!((e.line, e.col), (2, 8));
    assert_eq!(format!("{e}"), "line 2, column 8: expected a value");

    let e = parse("tru").unwrap_err();
    assert_eq!((e.line, e.col), (1, 1));

    let e = parse("[1,2,]").unwrap_err();
    assert_eq!((e.line, e.col), (1, 6));
}

#[test]
fn nesting_depth_is_bounded() {
    let deep_ok = format!("{}1{}", "[".repeat(100), "]".repeat(100));
    assert!(parse(&deep_ok).is_ok());

    let too_deep = format!("{}1{}", "[".repeat(200), "]".repeat(200));
    let e = parse(&too_deep).unwrap_err();
    assert_eq!(e.msg, "nesting too deep");
}

#[test]
fn nonfinite_floats_serialize_as_null() {
    let v = Value::Number(Number::Float(f64::NAN));
    assert_eq!(v.to_string(), "null");
    let v = Value::Array(vec![Value::Number(Number::Float(f64::INFINITY))]);
    assert_eq!(v.to_string(), "[null]");
}

#[test]
fn string_escapes_in_output() {
    let v = Value::String("a\"b\\c\nd\u{1}".to_string());
    assert_eq!(v.to_string(), "\"a\\\"b\\\\c\\nd\\u0001\"");
}
