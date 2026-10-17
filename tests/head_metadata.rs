//! The tab title and icon come from `index.html`, which every route is served
//! from, so a static check on that file covers the checkout page a customer
//! lands on as much as the dashboard. A missing icon file would not fail the
//! build by itself: the browser would just show its default.

use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn tab_title_is_random_cash() {
    let html = std::fs::read_to_string(root().join("index.html")).unwrap();
    assert!(html.contains("<title>Random Cash</title>"));
}

#[test]
fn favicons_are_linked_and_shipped() {
    let html = std::fs::read_to_string(root().join("index.html")).unwrap();
    for name in ["favicon.svg", "favicon.png", "favicon.ico"] {
        assert!(
            html.contains(&format!("rel=\"copy-file\" href=\"assets/{name}\"")),
            "index.html does not copy {name} into the bundle"
        );
        assert!(
            html.contains(&format!("href=\"/{name}\"")),
            "index.html does not link /{name}"
        );
        let len = std::fs::metadata(root().join("assets").join(name))
            .unwrap_or_else(|_| panic!("assets/{name} is missing"))
            .len();
        assert!(len > 0, "assets/{name} is empty");
    }
}
