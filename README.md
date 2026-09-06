# json-rs
Minimal **RFC 8259 JSON** parsing and serialization, built entirely on the
Rust standard library. **Zero dependencies.**

Rust's std has no JSON support; the usual answer is serde's crate tree. This
crate is the other end of the trade: a small `Value` enum, a strict
spec-anchored parser with line/column errors, and compact + pretty
serialization. Parse, inspect, serialize, done.

## Philosophy

See [nativelite-philosophy](https://github.com/nativelite/nativelite-philosophy) for the broader engineering standards and attack surface reduction strategy behind all nativelite packages.

## Usage

```rust
let v = json::parse(r#"{"name": "ada", "tags": [1, 2]}"#).unwrap();

v.get("name").and_then(|v| v.as_str());   // Some("ada")
v.to_string();                            // {"name":"ada","tags":[1,2]}
v.pretty(2);                              // indented, one member per line
```

`parse(&str) -> Result<Value, ParseError>` is the whole entry point.
`ParseError` carries a 1-based `line` and `col` and displays as
`line 2, column 8: expected a value`.

## Design decisions, stated plainly

- **Objects preserve order**: a vector of pairs, not a hash map. Duplicate
  keys survive parse → serialize; `get` returns the *last* occurrence,
  matching `JSON.parse`.
- **Numbers**: `Number::Int(i64)` when the lexeme is integral and fits (so
  IDs beyond 2^53 stay exact), else `Number::Float(f64)`. Values that
  overflow `f64` (`1e999`) are a parse error, not infinity.
- **Strict by default**: trailing commas, comments, single quotes, unquoted
  keys, `NaN`/`Infinity`, leading zeros, raw control characters in strings,
  and unpaired surrogates are rejected. One leading BOM is skipped. Nesting
  is bounded (`MAX_DEPTH = 128`) so hostile input can't blow the stack.
- **Float output** uses Rust's shortest round-trip formatting with `.0` kept
  on integral floats, so a float stays a float across a round trip.

## What's deliberately out of scope

- **Typed mapping / derive.** No serde replacement; you get a `Value` and
  walk it.
- **Streaming / SAX.** The input is a complete text.
- **Extensions.** No JSON5, no comments, no trailing commas. We parse RFC
  8259 and only RFC 8259.

## Command line

The crate ships a tiny `json` binary, a validator and (pretty-)printer:

```bash
some-llm-call | json            # pretty-print stdin
some-llm-call | json --compact  # minify
json --check < config.json      # validate only; line/col on stderr if bad
```

## Correctness

Tests are table-driven and anchored to RFC 8259 and the failure classes
catalogued by JSONTestSuite: ~45 valid inputs asserted **byte-exact** against
their compact serialization (escapes, surrogate pairs, BOM, number edge
cases including `i64::MAX`/`MIN` and the 2^53+1 precision trap), ~60 inputs
that must be rejected (structure, numbers, literals, comments, strings, lone
surrogates), round-trip stability over the whole valid table, pretty-print
goldens, exact error line/column positions, and the depth bound.

## Development

```bash
python dev.py check   # zero-dependency guard + cargo test (the pre-push gate)
python dev.py test    # cargo test
python dev.py fmt     # cargo fmt --check
python dev.py guard   # zero-dependency guard
```

`dev.py` is a stdlib-only runner, so `python dev.py check` is the same
one-command local gate used across every nativelite package. The guard fails
if `Cargo.toml` declares any dependency: runtime, build, or dev.
