//! The tab title and icon come from `index.html`, which every route is served
//! from, so a static check on that file covers the checkout page a customer
//! lands on as much as the dashboard. A missing icon file would not fail the
//! build by itself: the browser would just show its default.

use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn tab_title_is_agent8_cash() {
    let html = std::fs::read_to_string(root().join("index.html")).unwrap();
    assert!(html.contains("<title>agent8.cash</title>"));
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

/// robots.txt and llms.txt are fetched directly by URL convention, not via
/// any tag a browser acts on - so unlike the favicons above, there is no
/// second `href="/..."` reference to check. The copy-file tag is the only
/// thing that makes trunk ship them at the site root at all.
#[test]
fn robots_and_llms_txt_are_shipped_at_the_site_root() {
    let html = std::fs::read_to_string(root().join("index.html")).unwrap();
    for name in ["robots.txt", "llms.txt"] {
        assert!(
            html.contains(&format!("rel=\"copy-file\" href=\"assets/{name}\"")),
            "index.html does not copy {name} into the bundle"
        );
    }

    let robots = std::fs::read_to_string(root().join("assets/robots.txt")).unwrap();
    assert!(
        robots.contains("User-agent:"),
        "assets/robots.txt is not a real robots.txt"
    );
    assert!(
        !robots.to_lowercase().contains("disallow: /"),
        "assets/robots.txt must not blanket-disallow crawling - agents are meant to be welcome here"
    );

    let llms = std::fs::read_to_string(root().join("assets/llms.txt")).unwrap();
    assert!(
        llms.starts_with('#'),
        "assets/llms.txt should start with a top-level heading, per the llms.txt convention"
    );
    assert!(
        llms.contains("docs.agent8.cash"),
        "assets/llms.txt should point agents at the real documentation"
    );
}
