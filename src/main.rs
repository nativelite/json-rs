//! Command-line entry point: `json`.
//!
//! Reads JSON from stdin; pretty-prints by default, `--compact` minifies,
//! `--check` validates silently. Invalid input exits 1 with a line/column
//! message on stderr, a drop-in validator for pipelines.
//!
//! ```text
//! some-llm-call | json            # pretty-print
//! some-llm-call | json --compact  # minify
//! json --check < config.json      # validate only
//! ```

use std::io::Read;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let mut compact = false;
    let mut check = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--compact" => compact = true,
            "--check" => check = true,
            _ => {
                eprintln!("usage: json [--compact | --check] < input.json");
                return 2;
            }
        }
    }
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("json: stdin is not valid UTF-8");
        return 1;
    }
    match json::parse(&input) {
        Ok(v) => {
            if check {
                // valid: say nothing, exit 0
            } else if compact {
                println!("{v}");
            } else {
                println!("{}", v.pretty(2));
            }
            0
        }
        Err(e) => {
            eprintln!("json: {e}");
            1
        }
    }
}
