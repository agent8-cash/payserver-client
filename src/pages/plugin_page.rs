//! Draws a plugin's page.
//!
//! Nothing in this file knows what any plugin does. A plugin ships a
//! [`PageElement`] tree and this renders it, which is the only arrangement
//! that works: a plugin cannot ship Rust into an already-compiled client,
//! and this client is public while some plugins are not.
//!
//! # Two rules that are not cosmetic
//!
//! **An unrecognised element still draws something.** The vocabulary grows,
//! and a plugin built against a newer server will send elements this build
//! has never heard of. `PageElement::Unknown` is what that becomes, and it
//! renders a visible placeholder - because the alternative is a blank space
//! where a merchant expected to see what they owe, and a silently missing
//! panel beside a paywall is the worst thing this renderer could produce.
//!
//! **A link is checked before it is followed.** A plugin names where its
//! button goes, and a plugin is not a trusted source of somewhere to send a
//! merchant with money in hand. Only a host-relative path is honoured; an
//! absolute URL, a scheme, or a protocol-relative `//host` is treated as no
//! link at all, and the button draws disabled.

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use payserver_plugin_api::page::{
    Badge, Button, ButtonVariant, Card, Direction, Fields, Figure, Form, Grid, Input, Notice,
    PageElement, Row, Section, Select, Stack, Tab, Table, Tabs, Text, TextStyle, Tone,
};

use crate::api::ApiClient;
use crate::components::LoadingState;

/// Whether a plugin-supplied link may be followed.
///
/// Only a path on this origin. Everything else is refused, and refused
/// silently in the direction that cannot hurt: the button renders disabled
/// rather than pointing somewhere else.
///
/// `//evil.example` is the case worth naming. It has no scheme, so a check
/// for `http://` misses it, and a browser reads it as protocol-relative and
/// goes to another host.
#[must_use]
pub fn safe_href(href: &str) -> Option<&str> {
    let trimmed = href.trim();
    if trimmed.starts_with('/') && !trimmed.starts_with("//") {
        Some(trimmed)
    } else {
        None
    }
}

fn tone_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Neutral => "badge badge-neutral",
        Tone::Info => "badge badge-info",
        Tone::Success => "badge badge-success",
        Tone::Warning => "badge badge-warning",
        Tone::Danger => "badge badge-error",
    }
}

fn figure_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Neutral => "plugin-figure plugin-figure-neutral",
        Tone::Info => "plugin-figure plugin-figure-info",
        Tone::Success => "plugin-figure plugin-figure-success",
        Tone::Warning => "plugin-figure plugin-figure-warning",
        Tone::Danger => "plugin-figure plugin-figure-danger",
    }
}

fn notice_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Neutral => "plugin-notice plugin-notice-neutral",
        Tone::Info => "plugin-notice plugin-notice-info",
        Tone::Success => "plugin-notice plugin-notice-success",
        Tone::Warning => "plugin-notice plugin-notice-warning",
        Tone::Danger => "plugin-notice plugin-notice-danger",
    }
}

fn button_class(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "btn btn-primary",
        ButtonVariant::Secondary => "btn btn-secondary",
        ButtonVariant::Danger => "btn btn-danger",
        ButtonVariant::Ghost | ButtonVariant::Outline => "btn btn-ghost",
    }
}

/// One element and everything under it.
fn render(element: &PageElement) -> AnyView {
    match element {
        PageElement::Badge(Badge { text, tone }) => view! {
            <span class=tone_class(*tone)>{text.clone()}</span>
        }
        .into_any(),

        // Prose. Drawn with the host's own type styles rather than anything
        // the plugin chose - a plugin says what the text is doing, never how
        // big it is, so a plugin page cannot drift away from the screens
        // beside it.
        PageElement::Text(Text { text, style }) => {
            let class = match style {
                TextStyle::Body => "plugin-text",
                TextStyle::Muted => "plugin-text plugin-text-muted",
                TextStyle::Strong => "plugin-text plugin-text-strong",
            };
            view! { <p class=class>{text.clone()}</p> }.into_any()
        }

        // Label-and-value pairs, as a definition list. A `<dl>` because that
        // is what this is, and because it lets the stylesheet collapse to one
        // column on a narrow screen without the renderer knowing the width.
        PageElement::Fields(Fields { fields }) => {
            let rows = fields
                .iter()
                .map(|field| {
                    view! {
                        <div class="plugin-field-row">
                            <dt class="plugin-field-key">{field.label.clone()}</dt>
                            <dd class="plugin-field-value">{field.value.clone()}</dd>
                        </div>
                    }
                })
                .collect_view();
            view! { <dl class="plugin-fields">{rows}</dl> }.into_any()
        }

        PageElement::Notice(Notice { text, tone }) => view! {
            <div class=notice_class(*tone) role="status">{text.clone()}</div>
        }
        .into_any(),

        PageElement::Button(Button {
            label,
            variant,
            href,
        }) => {
            let class = button_class(*variant);
            match href.as_deref().and_then(safe_href) {
                Some(target) => view! {
                    <a class=class href=target.to_string()>{label.clone()}</a>
                }
                .into_any(),
                // No action system, and no link we will follow: the control
                // exists on the page, so it is drawn - visibly unavailable
                // rather than live and inert.
                None => view! {
                    <button class=class disabled=true>{label.clone()}</button>
                }
                .into_any(),
            }
        }

        // `ps-card`, not `card`. Both are defined, and they are not the same:
        // `ps-card` clips its children and `ps-card-header` styles its own
        // `h3`, which is why the title needs no class of its own. The rest of
        // this client draws 41 cards the `ps-` way and this renderer drew the
        // only one that did not - a plugin's page sat beside screens it did
        // not match, in the one place a merchant is asked for money.
        PageElement::Card(Card {
            title,
            badge,
            children,
        }) => {
            let children = render_all(children);
            // The header is a flex row with `space-between`, which is what
            // makes this a status *on* the card rather than the first thing
            // in it - the same slot the invoice detail page puts its payment
            // count in. A badge with no title still gets one, right-aligned,
            // because a status with nothing to be about is still a status.
            let header = (title.is_some() || badge.is_some()).then(|| {
                let badge = badge.clone().map(|badge| {
                    view! { <span class=tone_class(badge.tone)>{badge.text}</span> }
                });
                view! {
                    <div class="ps-card-header">
                        <h3>{title.clone().unwrap_or_default()}</h3>
                        {badge}
                    </div>
                }
            });
            view! {
                <div class="ps-card">
                    {header}
                    <div class="ps-card-body">{children}</div>
                </div>
            }
            .into_any()
        }

        PageElement::Section(Section { title, children }) => {
            let children = render_all(children);
            view! {
                <section class="plugin-section">
                    {title.clone().map(|t| view! { <h2 class="plugin-section-title">{t}</h2> })}
                    {children}
                </section>
            }
            .into_any()
        }

        PageElement::Table(Table { headers, rows }) => {
            let headers = headers.clone();
            let rows = rows.clone();
            view! {
                <div class="table-container">
                    <table class="table">
                        <thead>
                            <tr>{headers.into_iter().map(|h| view! { <th>{h}</th> }).collect_view()}</tr>
                        </thead>
                        <tbody>
                            {rows
                                .into_iter()
                                .map(|cells| view! {
                                    <tr>{cells.into_iter().map(|c| view! { <td>{c}</td> }).collect_view()}</tr>
                                })
                                .collect_view()}
                        </tbody>
                    </table>
                </div>
            }
            .into_any()
        }

        PageElement::Grid(Grid { columns, children }) => {
            let children = render_all(children);
            let style = format!(
                "display:grid;grid-template-columns:repeat({},minmax(0,1fr));gap:var(--space-3,12px)",
                (*columns).max(1)
            );
            view! { <div class="plugin-grid" style=style>{children}</div> }.into_any()
        }

        PageElement::Stack(Stack {
            direction,
            children,
        }) => {
            let children = render_all(children);
            let class = match direction {
                Direction::Column => "plugin-stack plugin-stack-column",
                Direction::Row => "plugin-stack plugin-stack-row",
            };
            view! { <div class=class>{children}</div> }.into_any()
        }

        PageElement::Row(Row { children }) => {
            let children = render_all(children);
            view! { <div class="plugin-stack plugin-stack-row">{children}</div> }.into_any()
        }

        PageElement::Form(Form { children }) => {
            let children = render_all(children);
            // No actions in the vocabulary, so this never submits. It is a
            // layout container that happens to be called a form.
            view! { <div class="plugin-form">{children}</div> }.into_any()
        }

        PageElement::Input(Input { label, placeholder }) => {
            let placeholder = placeholder.clone();
            view! {
                <label class="plugin-field">
                    {label.clone().map(|l| view! { <span class="plugin-field-label">{l}</span> })}
                    <input class="form-input" placeholder=placeholder disabled=true />
                </label>
            }
            .into_any()
        }

        PageElement::Select(Select { label, options }) => {
            let options = options.clone();
            view! {
                <label class="plugin-field">
                    {label.clone().map(|l| view! { <span class="plugin-field-label">{l}</span> })}
                    <select class="form-input" disabled=true>
                        {options.into_iter().map(|o| view! { <option>{o}</option> }).collect_view()}
                    </select>
                </label>
            }
            .into_any()
        }

        PageElement::Tabs(Tabs { tabs }) => {
            // Rendered stacked rather than as real tabs: switching one is an
            // interaction, and nothing here can tell the plugin about it. All
            // of the content is on the page, which is the honest fallback -
            // hiding content behind a control that does not work would lose
            // it entirely.
            let sections = tabs
                .iter()
                .map(|Tab { label, content }| {
                    let content = render_all(content);
                    view! {
                        <section class="plugin-tab">
                            <h3 class="plugin-tab-label">{label.clone()}</h3>
                            {content}
                        </section>
                    }
                })
                .collect_view();
            view! { <div class="plugin-tabs">{sections}</div> }.into_any()
        }

        // A summary tile: the number carries the tone, in place, rather than a
        // pill beside it - the tone says how urgently the figure matters, not
        // that it is a status on something else.
        PageElement::Figure(Figure { label, value, tone }) => view! {
            <div class=figure_class(*tone)>
                <span class="plugin-figure-label">{label.clone()}</span>
                <span class="plugin-figure-value">{value.clone()}</span>
            </div>
        }
        .into_any(),

        PageElement::Unknown => view! {
            <div class="plugin-notice plugin-notice-warning" role="status">
                "This part of the page needs a newer version of the dashboard."
            </div>
        }
        .into_any(),
    }
}

fn render_all(children: &[PageElement]) -> Vec<AnyView> {
    children.iter().map(render).collect()
}

/// A plugin page, addressed by plugin id and path.
///
/// # The page around the page
///
/// A plugin ships the *contents* of a screen. Everything that makes it one of
/// this product's screens rather than a panel floating on a background - the
/// column, the gap, the width it stops at, the title at the top - belongs
/// here, because a plugin has no way to express any of it and should not.
///
/// It was missing. This rendered into `class="page"`, which the stylesheet
/// does not define, so a plugin page had no max width, no vertical rhythm and
/// no heading while every screen beside it had all three. That is the whole
/// of why these looked like they came from somewhere else.
///
/// The title comes from the plugin's own manifest - the same `label` the
/// sidebar entry is drawn from - so the heading on the page and the link that
/// reached it always say the same thing. Fetched alongside the page itself
/// rather than passed down, because a page reached by its URL directly has no
/// navigation state to have been passed anything by.
#[component]
pub fn PluginPageView() -> impl IntoView {
    let params = use_params_map();
    let api = expect_context::<Signal<ApiClient>>();

    let page = LocalResource::new(move || {
        let api = api.get();
        let plugin_id = params.with(|p| p.get("id").unwrap_or_default());
        let path = params.with(|p| p.get("path").unwrap_or_default());
        async move {
            if plugin_id.is_empty() || path.is_empty() {
                return Err(crate::api::ApiError::Parse("no page requested".to_string()));
            }

            // The heading is not worth failing the page over: a merchant who
            // can see what they owe under an untitled heading is better off
            // than one who sees an error because the nav list timed out.
            let title = api
                .list_plugin_pages()
                .await
                .unwrap_or_default()
                .into_iter()
                .find(|listed| listed.plugin_id == plugin_id && listed.path == path)
                .map(|listed| listed.label);

            api.get_plugin_page(&plugin_id, &path)
                .await
                .map(|element| (title, element))
        }
    });

    view! {
        <div class="ps-page">
            <Suspense fallback=|| view! { <LoadingState message="Loading…" /> }.into_any()>
                {move || Suspend::new(async move {
                    match page.await {
                        Ok((title, element)) => view! {
                            {title.map(|title| view! {
                                <div class="page-header-row">
                                    <div><h1 class="page-title">{title}</h1></div>
                                </div>
                            })}
                            {render(&element)}
                        }
                        .into_any(),
                        // The server answers 502 for a plugin that could not
                        // draw its page and 404 for one that has no such
                        // page. Neither is something a merchant can act on,
                        // so both say the same thing rather than exposing
                        // which.
                        Err(_) => view! {
                            <div class="empty-state">
                                <div class="empty-state-title">"This page is unavailable"</div>
                                <div class="empty-state-description">
                                    "It could not be loaded just now. Try again shortly."
                                </div>
                            </div>
                        }
                        .into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use leptos::prelude::RenderHtml;

    /// Tags stripped, so an assertion can check for real text content rather
    /// than markup that merely mentions it in an attribute.
    fn strip_tags(html: &str) -> String {
        let mut out = String::with_capacity(html.len());
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => out.push(c),
                _ => {}
            }
        }
        out
    }

    /// The stylesheet, read at compile time so the check below is against the
    /// file that actually ships.
    const STYLES: &str = include_str!("../../styles.css");

    /// This file, likewise, so the class names in its markup are checked
    /// rather than a list of them that someone has to remember to update.
    const SOURCE: &str = include_str!("plugin_page.rs");

    /// Whether `styles.css` has any rule that could match `class`.
    ///
    /// Looks for the selector as a whole token - `.ps-card` must not be
    /// satisfied by `.ps-card-body` - and accepts it anywhere a selector may
    /// legally end: a brace, a comma, whitespace, a combinator, a pseudo, or
    /// another class in a compound selector.
    fn is_defined(class: &str) -> bool {
        // Comments stripped first. This stylesheet contains a comment that
        // mentions `.page` by name - written while fixing the very bug this
        // guard exists to catch - and a scan that counted it would report the
        // class as defined because someone described it.
        let styles = strip_comments(STYLES);
        let needle = format!(".{class}");
        let mut from = 0;
        while let Some(at) = styles[from..].find(&needle) {
            let start = from + at;
            let after = styles[start + needle.len()..].chars().next();
            let before = styles[..start].chars().next_back();
            // Not preceded by an identifier character, or `.ps-page` would be
            // found inside `.x.ps-page` only - which is fine - but also
            // inside a longer name it is not part of.
            let boundary_before =
                before.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
            let boundary_after =
                after.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
            if boundary_before && boundary_after {
                return true;
            }
            from = start + 1;
        }
        false
    }

    /// `/* ... */` removed, so a class named in prose is not mistaken for a
    /// class that is styled.
    fn strip_comments(css: &str) -> String {
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        while let Some(open) = rest.find("/*") {
            out.push_str(&rest[..open]);
            match rest[open + 2..].find("*/") {
                Some(close) => rest = &rest[open + 2 + close + 2..],
                None => return out,
            }
        }
        out.push_str(rest);
        out
    }

    /// Every class this renderer puts in the markup must exist in the
    /// stylesheet.
    ///
    /// This is the check that was missing. The page shell was
    /// `class="page"` - a class `styles.css` does not define at all - so a
    /// plugin page had no max width, no column and no gap while every screen
    /// beside it had all three, and nothing anywhere said so. A class name is
    /// a reference to a rule, and a reference that resolves to nothing is the
    /// one kind of styling mistake that is completely silent: the markup is
    /// valid, the build is green, and the page is simply unstyled.
    #[test]
    fn every_class_this_renderer_emits_is_defined_in_the_stylesheet() {
        let mut missing = Vec::new();

        // The literal `class` attributes in the markup above, with this
        // file's own comments dropped first - the doc comments here quote
        // class names while explaining them.
        let code: String = SOURCE
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut rest = code.as_str();
        while let Some(at) = rest.find("class=\"") {
            rest = &rest[at + 7..];
            let Some(end) = rest.find('"') else { break };
            let (value, tail) = rest.split_at(end);
            rest = tail;
            for class in value.split_whitespace() {
                if !is_defined(class) {
                    missing.push(class.to_string());
                }
            }
        }

        // And the ones chosen by a function, taken by calling it rather than
        // by reading it - a variant added to any of these enums shows up here
        // without anyone remembering to extend a list.
        let from_functions = [
            Tone::Neutral,
            Tone::Info,
            Tone::Success,
            Tone::Warning,
            Tone::Danger,
        ]
        .into_iter()
        .flat_map(|tone| [tone_class(tone), notice_class(tone), figure_class(tone)])
        .chain(
            [
                ButtonVariant::Primary,
                ButtonVariant::Secondary,
                ButtonVariant::Danger,
                ButtonVariant::Ghost,
                ButtonVariant::Outline,
            ]
            .into_iter()
            .map(button_class),
        );
        for value in from_functions {
            for class in value.split_whitespace() {
                if !is_defined(class) {
                    missing.push(class.to_string());
                }
            }
        }

        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "these classes are emitted by the plugin page renderer and defined nowhere \
             in styles.css, so they style nothing: {missing:?}"
        );
    }

    /// The check above only means something if it can fail.
    #[test]
    fn a_class_the_stylesheet_does_not_define_is_detected() {
        assert!(
            !is_defined("definitely-not-a-class-in-this-stylesheet"),
            "the detector must not report an undefined class as present"
        );
        assert!(is_defined("ps-page"), "and must find one that is present");
        assert!(
            !is_defined("page"),
            "`.page` is exactly the class this renderer used to emit and the \
             stylesheet has never defined; if this starts passing, the guard above \
             has stopped guarding"
        );
    }

    /// A plugin is not a trusted source of somewhere to send a merchant who
    /// is about to pay. Only a path on this origin is followed.
    #[test]
    fn only_host_relative_paths_are_followed() {
        assert_eq!(safe_href("/checkout/9f3a"), Some("/checkout/9f3a"));
        assert_eq!(safe_href("  /checkout/9f3a  "), Some("/checkout/9f3a"));

        assert_eq!(safe_href("https://evil.example/pay"), None);
        assert_eq!(safe_href("http://evil.example/pay"), None);
        assert_eq!(safe_href("javascript:alert(1)"), None);
        assert_eq!(
            safe_href("//evil.example/pay"),
            None,
            "protocol-relative has no scheme to match on and still leaves the origin"
        );
        assert_eq!(safe_href("checkout/9f3a"), None, "a bare relative path");
        assert_eq!(safe_href(""), None);
    }

    /// The module doc calls a silently blank panel beside a paywall "the
    /// worst thing this renderer could produce". Checked against markup a
    /// browser would actually receive, not the `view!` literal by eye.
    ///
    /// `render` is the same private helper `PluginPageView` calls above to
    /// draw the page mounted at `/plugins/:id/:path` in `app/mod.rs` - this
    /// exercises the renderer that ships, not a revived one.
    ///
    /// `PageElement::Unknown` is not just constructible in a test: the type
    /// is `#[serde(tag = "type", ...)]` with `#[serde(other)]` on `Unknown`
    /// (`payserver-plugin-api/src/page.rs`), so any `type` string a plugin
    /// sends that predates this client's vocabulary deserializes into it on
    /// the real network path, `ApiClient::get_plugin_page` in
    /// `api/client/plugins.rs`. This test exercises what that path produces
    /// once it reaches `render`, not a value only test code can build.
    #[test]
    fn an_unrecognised_element_renders_a_visible_placeholder() {
        let html = render(&PageElement::Unknown).to_html();
        assert!(html.contains("plugin-notice plugin-notice-warning"));
        assert!(html.contains(r#"role="status""#));
        assert!(html.contains("This part of the page needs a newer version of the dashboard."));
    }

    // The fifteen variants below were in the same position `Unknown` was
    // before the test above: shipped, read, and never executed. Same three
    // assertions each - classes, the element's own contract, and that its
    // text actually reaches the markup - against `render`'s real output, not
    // the `view!` literal by eye.
    //
    // Ablated as a sample rather than all fifteen: `a_badge_renders_...` and
    // `a_table_renders_...` below, one assertion broken at a time (wrong
    // class, wrong tag/structure, blanked text), each confirmed to fail on
    // its own before the source was restored.

    #[test]
    fn a_badge_renders_its_tone_class_and_text() {
        let html = render(&PageElement::Badge(Badge {
            text: "Beta".to_string(),
            tone: Tone::Info,
        }))
        .to_html();
        assert!(html.contains("badge badge-info"));
        assert!(html.contains("<span"), "a badge is inline, not a block");
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_figure_renders_its_label_value_and_tone_class() {
        let html = render(&PageElement::Figure(Figure {
            label: "Lapsed".to_string(),
            value: "7".to_string(),
            tone: Tone::Danger,
        }))
        .to_html();
        assert!(html.contains("plugin-figure plugin-figure-danger"));
        let text = strip_tags(&html);
        assert!(text.contains("Lapsed") && text.contains('7'));
        assert!(!html.contains("needs a newer version"));
        // The label and the number are separate elements, because the
        // stylesheet tones the number and leaves the label alone. One combined
        // string would tone both, or neither.
        assert!(html.contains("plugin-figure-label") && html.contains("plugin-figure-value"));
    }

    /// Every tone, including the default one.
    ///
    /// `Tone::Neutral` is the reason this exists separately from the test
    /// above. It is `#[default]`, so a figure that says nothing about its tone
    /// gets it, and it was the one tone whose class the stylesheet did not
    /// define - the single test case above happened to use `Danger` and could
    /// not see it. A tone a plugin reaches by omission is the easiest one to
    /// leave untested and the likeliest one to be sent.
    #[test]
    fn every_tone_gives_a_figure_its_own_class_and_none_is_the_placeholder() {
        let mut seen = Vec::new();
        for tone in [
            Tone::Neutral,
            Tone::Info,
            Tone::Success,
            Tone::Warning,
            Tone::Danger,
        ] {
            let html = render(&PageElement::Figure(Figure {
                label: "Active".to_string(),
                value: "128".to_string(),
                tone,
            }))
            .to_html();
            assert!(
                !html.contains("needs a newer version"),
                "{tone:?} fell through to the unrecognised-element placeholder"
            );
            let class = figure_class(tone);
            assert!(html.contains(class), "{tone:?} should render {class}");
            seen.push(class);
        }

        // A tone that shares another's class renders as that other tone, which
        // is a wrong reading rather than a missing one - worse, and invisible
        // to a test that only checks the class is present.
        let mut unique = seen.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            seen.len(),
            "two tones render the same class, so one reads as the other: {seen:?}"
        );
    }

    /// `Figure` is a figure, not a badge.
    ///
    /// The vocabulary's own doc comment is explicit that four counts drawn as
    /// coloured pills above a table "read as status on nothing", and that this
    /// element exists precisely so a plugin does not have to do that. Drawing
    /// one with the badge classes would satisfy every other assertion here -
    /// the text is present, a tone class is present, nothing is the
    /// placeholder - while putting back the thing the element was added to
    /// avoid.
    #[test]
    fn a_figure_is_not_drawn_as_a_badge() {
        let html = render(&PageElement::Figure(Figure {
            label: "Past due".to_string(),
            value: "6".to_string(),
            tone: Tone::Warning,
        }))
        .to_html();
        assert!(
            !html.contains("badge"),
            "a figure must not borrow the badge classes: a toned pill says this is \
             a status on something else, and the number here is the whole point"
        );
    }

    #[test]
    fn a_button_with_a_link_renders_as_an_anchor_to_it() {
        let html = render(&PageElement::Button(Button {
            label: "Pay now".to_string(),
            variant: ButtonVariant::Primary,
            href: Some("/checkout/9f3a".to_string()),
        }))
        .to_html();
        assert!(html.contains("btn btn-primary"));
        assert!(html.contains("<a ") && html.contains(r#"href="/checkout/9f3a""#));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_card_renders_a_header_over_a_body() {
        let html = render(&PageElement::Card(Card {
            title: Some("Balances".to_string()),
            badge: None,
            children: vec![PageElement::Text(Text {
                text: "Updated a moment ago".to_string(),
                style: TextStyle::Muted,
            })],
        }))
        .to_html();
        assert!(
            html.contains("ps-card")
                && html.contains("ps-card-header")
                && html.contains("ps-card-body")
        );
        assert!(html.contains("<h3>Balances</h3>"));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn fields_render_as_a_definition_list() {
        let html = render(&PageElement::Fields(Fields {
            fields: vec![payserver_plugin_api::page::Field {
                label: "Price".to_string(),
                value: "0.50 USDC every 30 days".to_string(),
            }],
        }))
        .to_html();
        assert!(
            html.contains("plugin-fields")
                && html.contains("plugin-field-key")
                && html.contains("plugin-field-value")
        );
        assert!(html.contains("<dl") && html.contains("<dt") && html.contains("<dd"));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_form_renders_its_children_without_being_submittable() {
        let html = render(&PageElement::Form(Form {
            children: vec![PageElement::Input(Input {
                label: Some("Amount".to_string()),
                placeholder: "0.00".to_string(),
            })],
        }))
        .to_html();
        assert!(html.contains("plugin-form"));
        assert!(
            !html.contains("<form"),
            "there is no action system - a plugin form must never render as a \
             submittable <form>, only as a layout container"
        );
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_grid_renders_its_column_count_into_the_grid_style() {
        let html = render(&PageElement::Grid(Grid {
            columns: 3,
            children: vec![PageElement::Text(Text {
                text: "cell".to_string(),
                style: TextStyle::Body,
            })],
        }))
        .to_html();
        assert!(html.contains("plugin-grid"));
        assert!(html.contains("grid-template-columns:repeat(3,"));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn an_input_renders_disabled_with_its_label() {
        let html = render(&PageElement::Input(Input {
            label: Some("Amount".to_string()),
            placeholder: "0.00".to_string(),
        }))
        .to_html();
        assert!(html.contains("plugin-field") && html.contains("form-input"));
        assert!(
            html.contains("<input") && html.contains("disabled"),
            "there is no action system yet - an input must not be editable"
        );
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_notice_renders_its_tone_class_and_status_role() {
        let html = render(&PageElement::Notice(Notice {
            text: "Read-only preview".to_string(),
            tone: Tone::Warning,
        }))
        .to_html();
        assert!(html.contains("plugin-notice plugin-notice-warning"));
        assert!(html.contains(r#"role="status""#));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_row_lays_its_children_out_horizontally() {
        let html = render(&PageElement::Row(Row {
            children: vec![PageElement::Text(Text {
                text: "cell".to_string(),
                style: TextStyle::Body,
            })],
        }))
        .to_html();
        assert!(html.contains("plugin-stack plugin-stack-row"));
        assert!(html.contains("<p") && html.contains("cell"));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_section_renders_a_heading_over_its_children() {
        let html = render(&PageElement::Section(Section {
            title: Some("Advanced".to_string()),
            children: vec![PageElement::Text(Text {
                text: "cell".to_string(),
                style: TextStyle::Body,
            })],
        }))
        .to_html();
        assert!(html.contains("plugin-section") && html.contains("plugin-section-title"));
        assert!(html.contains("<h2") && html.contains("Advanced"));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_select_renders_disabled_with_its_options() {
        let html = render(&PageElement::Select(Select {
            label: Some("Network".to_string()),
            options: vec!["Ethereum".to_string(), "Polygon".to_string()],
        }))
        .to_html();
        assert!(html.contains("plugin-field") && html.contains("form-input"));
        assert!(
            html.contains("<select") && html.contains("disabled") && html.contains("<option"),
            "there is no action system yet - a select must not be editable"
        );
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_stack_renders_its_direction_class() {
        let html = render(&PageElement::Stack(Stack {
            direction: Direction::Column,
            children: vec![PageElement::Text(Text {
                text: "cell".to_string(),
                style: TextStyle::Body,
            })],
        }))
        .to_html();
        assert!(html.contains("plugin-stack plugin-stack-column"));
        assert!(html.contains("<p") && html.contains("cell"));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn a_table_renders_header_and_row_structure() {
        let html = render(&PageElement::Table(Table {
            headers: vec!["Chain".to_string(), "Balance".to_string()],
            rows: vec![vec!["eip155:1".to_string(), "1.2".to_string()]],
        }))
        .to_html();
        assert!(html.contains("table-container") && html.contains(r#"class="table""#));
        assert!(
            html.contains("<thead")
                && html.contains("<th")
                && html.contains("<tbody")
                && html.contains("<td")
        );
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn tabs_render_every_tab_stacked_with_its_own_heading() {
        let html = render(&PageElement::Tabs(Tabs {
            tabs: vec![Tab {
                label: "Overview".to_string(),
                content: vec![PageElement::Text(Text {
                    text: "cell".to_string(),
                    style: TextStyle::Body,
                })],
            }],
        }))
        .to_html();
        assert!(html.contains("plugin-tabs") && html.contains("plugin-tab-label"));
        assert!(html.contains("<h3") && html.contains("Overview"));
        assert!(!strip_tags(&html).trim().is_empty());
    }

    #[test]
    fn text_renders_its_style_class() {
        let html = render(&PageElement::Text(Text {
            text: "Your subscription starts once the first invoice is paid.".to_string(),
            style: TextStyle::Strong,
        }))
        .to_html();
        assert!(html.contains("plugin-text plugin-text-strong"));
        assert!(html.contains("<p"));
        assert!(!strip_tags(&html).trim().is_empty());
    }
}
