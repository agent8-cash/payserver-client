//! A string literal in a Leptos view is a text node, and text nodes are
//! escaped, so an HTML entity written there renders as visible text instead of
//! a symbol. Write the character itself.

use std::path::Path;

fn scan(dir: &Path, hits: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scan(&path, hits);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let src = std::fs::read_to_string(&path).unwrap();
            for (i, line) in src.lines().enumerate() {
                let t = line.trim_start();
                if t.starts_with("//") {
                    continue;
                }
                if let Some(pos) = line.find("\"&#") {
                    let rest = &line[pos + 3..];
                    let digits = rest.trim_start_matches('x');
                    let end = digits.find(';');
                    if end.is_some_and(|e| {
                        e > 0 && digits[..e].chars().all(|c| c.is_ascii_hexdigit())
                    }) {
                        hits.push(format!("{}:{}", path.display(), i + 1));
                    }
                }
            }
        }
    }
}

#[test]
fn no_html_entity_string_literals_in_views() {
    let mut hits = Vec::new();
    scan(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut hits,
    );
    assert!(
        hits.is_empty(),
        "HTML entity in a string literal renders as literal text; use the character: {hits:?}"
    );
}
