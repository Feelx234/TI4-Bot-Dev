//! Writes whitespace-free copies of `content/*.json` to `$OUT_DIR/min/`, which `loader.rs`
//! compiles in instead of the files themselves.
//!
//! The files in `content/` are pretty-printed, some with CRLF line ends: 957 KB of which 273 KB is
//! indentation and line ends. The corpus is parsed into `serde_json::Value`s at start, so the text
//! is only a carrier; every digest of the corpus (`provenance::digest_of`) is of the parsed
//! records and is the same for both texts. The files stay as they are (`CHECKSUMS.sha256` hashes
//! them); only the copy that becomes part of the binary, native and wasm, is smaller.
use std::env;
use std::fs;
use std::path::PathBuf;

/// `json` without the whitespace outside of strings.
fn minify(json: &str) -> String {
    let mut out = String::with_capacity(json.len() * 3 / 4);
    let mut in_string = false;
    let mut escaped = false;
    for c in json.chars() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if !matches!(c, ' ' | '\t' | '\n' | '\r') {
            out.push(c);
        }
    }
    out
}

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("content");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("min");
    fs::create_dir_all(&out).unwrap();
    println!("cargo:rerun-if-changed=content");
    println!("cargo:rerun-if-changed=build.rs");
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let text = fs::read_to_string(&path).unwrap();
            fs::write(out.join(path.file_name().unwrap()), minify(&text)).unwrap();
        }
    }
}
