//! The schema of the features and the shards, as JSON (read by `tools/mcts/ti4mcts/schema.py`, which refuses a mismatch).

use crate::encode::{ADJ_BYTES, FA, FEATURE_VERSION, FG, FO, FP, FS, KIND_BUCKETS, MAX_OBJ, MAX_SYS, PTRS, SEATS};
use crate::shard::META;

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| quote(s)).collect::<Vec<_>>().join(", "))
}

/// The schema text. `names` are the column names of the five blocks (system, seat, objective, global, option), if known.
#[must_use]
pub fn json(names: Option<&[Vec<String>; 5]>) -> String {
    let empty: [Vec<String>; 5] = Default::default();
    let n = names.unwrap_or(&empty);
    let meta: Vec<String> = META.iter().map(|s| (*s).to_owned()).collect();
    format!(
        "{{\n  \"feature_version\": {FEATURE_VERSION},\n  \"max_sys\": {MAX_SYS},\n  \"seats\": {SEATS},\n  \"max_obj\": {MAX_OBJ},\n  \"ptrs\": {PTRS},\n  \"adj_bytes\": {ADJ_BYTES},\n  \"kind_buckets\": {KIND_BUCKETS},\n  \"fs\": {FS},\n  \"fp\": {FP},\n  \"fo\": {FO},\n  \"fg\": {FG},\n  \"fa\": {FA},\n  \"meta\": {},\n  \"names\": {{\n    \"sys\": {},\n    \"seat\": {},\n    \"obj\": {},\n    \"glob\": {},\n    \"opt\": {}\n  }}\n}}\n",
        list(&meta),
        list(&n[0]),
        list(&n[1]),
        list(&n[2]),
        list(&n[3]),
        list(&n[4]),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_schema_names_the_dimensions() {
        let text = super::json(None);
        for key in ["feature_version", "fs", "fp", "fo", "fg", "fa", "meta", "max_sys"] {
            assert!(text.contains(&format!("\"{key}\"")), "{key}");
        }
        assert_eq!(super::quote("a\"b\\"), "\"a\\\"b\\\\\"");
    }
}
