# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-08-28

### Added
- `json::parse(&str) -> Result<Value, ParseError>` — strict RFC 8259 parser:
  rejects trailing commas, comments, single quotes, unquoted keys,
  `NaN`/`Infinity`, leading zeros, raw control characters, unpaired
  surrogates, and out-of-`f64`-range numbers; skips one leading BOM; bounds
  nesting at `MAX_DEPTH = 128`. Errors carry 1-based line/column.
- `Value` enum with order-preserving objects (vec of pairs, duplicates kept;
  `get` is last-wins like `JSON.parse`) and `Number::Int(i64)` /
  `Number::Float(f64)` so large integers stay exact.
- Serialization: compact via `Display`, indented via `Value::pretty(n)`;
  floats use shortest round-trip formatting with `.0` kept on integral
  floats; non-finite floats (API-only) serialize as `null`.
- Accessors: `as_bool`, `as_i64`, `as_f64`, `as_str`, `as_array`,
  `as_object`, `get`.
- `json` binary: pretty-print stdin, `--compact` to minify, `--check` to
  validate with line/column errors on stderr.
- Table-driven test suite anchored to RFC 8259 / JSONTestSuite failure
  classes: byte-exact serialization of valid vectors, must-reject vectors,
  round-trip stability, pretty goldens, error positions, depth bound.
- Stdlib-only `dev.py` runner (`check`, `test`, `fmt`, `guard`) and the
  Cargo.toml zero-dependency guard.

First package in the nativelite **agent terminal** suite (see
`roadmap/agent-terminal-suite.md` in `nativelite/ops`).

[Unreleased]: https://github.com/nativelite/json-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/nativelite/json-rs/releases/tag/v0.1.0
