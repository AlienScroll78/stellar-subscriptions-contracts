//! Checks that `docs/errors.json` matches the `#[repr(u32)]` error codes
//! declared in each contract's `errors.rs`.
//!
//! The JSON is parsed line by line (one entry per line) so no extra
//! dependency is needed.

extern crate std;

use std::collections::BTreeSet;
use std::string::{String, ToString};

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pat = std::format!("\"{}\": ", key);
    let start = line.find(&pat)? + pat.len();
    let rest = &line[start..];
    let end = rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches('"'))
}

fn rust_codes(contract: &str) -> BTreeSet<(String, u32)> {
    let path = std::format!(
        "{}/../{}/src/errors.rs",
        env!("CARGO_MANIFEST_DIR"),
        contract
    );
    let src = std::fs::read_to_string(path).unwrap();
    let mut out = BTreeSet::new();
    for line in src.lines() {
        let t = line.trim().trim_end_matches(',');
        if let Some((name, code)) = t.split_once(" = ") {
            if let Ok(code) = code.trim().parse::<u32>() {
                if name.chars().all(|c| c.is_ascii_alphanumeric()) {
                    out.insert((name.to_string(), code));
                }
            }
        }
    }
    out
}

#[test]
fn errors_json_codes_match_rust_source() {
    let path = std::format!("{}/../../docs/errors.json", env!("CARGO_MANIFEST_DIR"));
    let json = std::fs::read_to_string(path).unwrap();
    let mut seen: BTreeSet<(String, u32)> = BTreeSet::new();
    let mut count = 0;
    for line in json.lines().filter(|l| l.contains("\"contract\"")) {
        let contract = field(line, "contract").unwrap();
        let code: u32 = field(line, "code").unwrap().parse().unwrap();
        let name = field(line, "name").unwrap();
        assert!(
            rust_codes(contract).contains(&(name.to_string(), code)),
            "{contract}::{name} = {code} not found in errors.rs"
        );
        assert!(
            seen.insert((contract.to_string(), code)),
            "duplicate code {code} for {contract}"
        );
        count += 1;
    }
    assert!(count > 0);
}
