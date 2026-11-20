//! The issues list under the source area: one row per thing a writer should
//! fix or know about, each naming its line and moving the caret there.

use domain::song_library::import::{IssueKind, MarkdownIssue};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Role, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::*, px, rgb,
};
use ui::{assets::icon, environment::UiEnvironment, primitives::focus_visible, tokens};

use super::MarkdownDraftView;

/// The heading of the list (Decision 24).
pub(super) const HEADING: &str = "Issues";

/// What an issue says, in operator words (Decision 22).
pub(super) fn message(kind: &IssueKind) -> String {
    match kind {
        IssueKind::NotUtf8 => "This file isn't UTF-8 text.".to_owned(),
        IssueKind::NoFrontMatter => "Add front matter with a title.".to_owned(),
        IssueKind::BadFrontMatter(reason) => reason.clone(),
        IssueKind::MissingTitle => "Add a title.".to_owned(),
        IssueKind::UnknownKey(key) => format!("Ordo doesn't use {key}; it's ignored."),
        IssueKind::UnlistedLanguage(value) => format!("{value} isn't one of Ordo's languages."),
        IssueKind::NoLanguage => "Add a language: one of Ordo's languages.".to_owned(),
        IssueKind::UnresolvedRepeat(label) => format!("No section is called {label}."),
        IssueKind::AmbiguousRepeat { label, sections } => {
            format!("{sections} sections are called {label}. Rename one.")
        }
        IssueKind::NoWords(Some(label)) => format!("{label} has no words yet."),
        IssueKind::NoWords(None) => "This section has no words yet.".to_owned(),
        IssueKind::NotALabel(text) => format!("{text} isn't a section label; it's kept as lyrics."),
    }
}

/// The row's text: "Line {n} \u{b7} {message}".
pub(super) fn row_text(issue: &MarkdownIssue) -> String {
    format!("Line {} \u{b7} {}", issue.line, message(&issue.kind))
}

/// The byte offset where 1-based `line` starts in `text`; the end of the text
/// when the file has fewer lines.
pub(super) fn line_start(text: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    text.char_indices()
        .filter(|(_, found)| *found == '\n')
        .nth(line - 2)
        .map_or(text.len(), |(at, _)| at + 1)
}

/// The issues list. Always drawn, so the region has content even when the
/// file is clean.
pub(super) fn render(
    view: &MarkdownDraftView,
    environment: UiEnvironment,
    cx: &mut Context<MarkdownDraftView>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let rows = view
        .parsed
        .issues
        .iter()
        .enumerate()
        .map(|(at, issue)| {
            let text = row_text(issue);
            let line = issue.line;
            let blocking = issue.blocking;
            let go = MarkdownDraftView::act(cx, move |view, window, cx| {
                view.go_to_line(line, window, cx);
            });
            let key = MarkdownDraftView::act(cx, move |view, window, cx| {
                view.go_to_line(line, window, cx);
            });
            let colour = if blocking {
                palette.danger
            } else {
                palette.text
            };
            let row = div()
                .id(("markdown-issue", at))
                .debug_selector(move || format!("markdown-issue-{at}"))
                .role(Role::ListItem)
                .aria_label(SharedString::from(text.clone()))
                .focusable()
                .tab_stop(true)
                .flex_none()
                .flex()
                .items_start()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .w_full()
                .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                .py(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .border_b(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.border))
                .text_color(rgb(colour))
                .cursor_pointer()
                .on_click(move |_, window, cx| go(window, cx))
                .on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "enter" {
                        key(window, cx);
                        cx.stop_propagation();
                    }
                })
                .when(blocking, |row| {
                    row.child(
                        gpui::svg()
                            .path(icon::CIRCLE_X)
                            .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
                            .flex_none()
                            .text_color(rgb(palette.danger)),
                    )
                })
                .child(div().flex_1().min_w_0().child(text));
            focus_visible(row, environment, palette.background)
        })
        .collect::<Vec<_>>();
    div()
        .debug_selector(|| "markdown-issues".into())
        .flex()
        .flex_col()
        .w_full()
        .child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .pb(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(HEADING),
        )
        .children(rows)
        .into_any_element()
}
