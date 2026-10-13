//! `e2e-routes.txt` is what the end-to-end suite walks. The suite cannot import
//! the Rust router, so a crawl of the rendered DOM is the only other way to find
//! pages, and it never sees one reached from a card or a tab. This test derives
//! the route set from the router's own source and fails when the checked-in
//! list differs, so the list cannot rot quietly.

use std::collections::BTreeSet;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The `path!("...")` argument on a line, if it has one.
fn path_arg(line: &str) -> Option<&str> {
    let rest = &line[line.find("path!(\"")? + "path!(\"".len()..];
    Some(&rest[..rest.find('"')?])
}

/// Full paths of every `<Route>`, composing the `<ParentRoute>` segments
/// around it.
fn router_routes(src: &str) -> BTreeSet<String> {
    let mut stack: Vec<&str> = Vec::new();
    let mut out = BTreeSet::new();
    for line in src.lines().map(str::trim) {
        if line.starts_with("//") {
            continue;
        }
        if line.starts_with("</ParentRoute") {
            stack.pop();
        } else if line.starts_with("<ParentRoute") {
            stack.push(path_arg(line).expect("ParentRoute without a path!()"));
        } else if line.starts_with("<Route ") {
            let seg = path_arg(line).expect("Route without a path!()");
            let joined: String = stack.iter().chain([&seg]).fold(String::new(), |acc, s| {
                format!("{}{}", acc.trim_end_matches('/'), s)
            });
            out.insert(if joined.is_empty() {
                "/".into()
            } else {
                joined
            });
        }
    }
    out
}

fn listed_routes(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

#[test]
fn route_list_matches_the_router() {
    let src = std::fs::read_to_string(root().join("src/app/mod.rs")).unwrap();
    let list = std::fs::read_to_string(root().join("e2e-routes.txt")).unwrap();
    let router = router_routes(&src);
    let listed = listed_routes(&list);

    // A parser that matches nothing would make both differences empty only if
    // the list were empty too; require real content so that cannot read as in sync.
    assert!(
        router.len() >= 10,
        "extracted only {router:?} from the router - the parser no longer matches"
    );

    let unlisted: Vec<_> = router.difference(&listed).collect();
    let stale: Vec<_> = listed.difference(&router).collect();
    assert!(
        unlisted.is_empty() && stale.is_empty(),
        "e2e-routes.txt and the router disagree.\n  in router, not in list: {unlisted:?}\n  in list, not in router: {stale:?}"
    );
}

#[test]
fn composes_nested_paths() {
    let src = r#"
        <Route path=path!("/a") view=A />
        <ParentRoute path=path!("/") view=L>
            <Route path=path!("") view=B />
            <ParentRoute path=path!("/x") view=O>
                <Route path=path!("") view=C />
                <Route path=path!("/y/:id") view=D />
            </ParentRoute>
        </ParentRoute>
    "#;
    let got: Vec<_> = router_routes(src).into_iter().collect();
    assert_eq!(got, ["/", "/a", "/x", "/x/y/:id"]);
}
