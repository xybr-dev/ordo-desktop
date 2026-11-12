//! The nested preview of an existing Song: identity and a compact Arrangement
//! summary only. No lyrics and nothing to edit.

use domain::identity::SongId;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px, rgb,
};
use ui::{environment::UiEnvironment, overlay::Overlay, tokens};

use super::{NewSongPanel, Outcome};

pub(super) const DESCRIPTION: &str = "Existing Song \u{b7} preview only";

/// The preview slideover for `song`, or `None` when the library no longer has
/// it.
pub(super) fn overlay(
    panel: &NewSongPanel,
    song: SongId,
    cx: &mut Context<NewSongPanel>,
) -> Option<Overlay> {
    let title = panel
        .store
        .read(cx)
        .library()
        .song(song)
        .ok()?
        .title()
        .to_owned();
    let body = cx.entity();
    let close = cx.entity();
    let report = panel.report.clone();
    Some(
        Overlay::slideover(title)
            .description(DESCRIPTION)
            .body(move |environment, _, cx| {
                body.update(cx, |panel, cx| render(panel, song, environment, cx))
            })
            .primary("Use this existing Song", move |window, cx| {
                report(Outcome::UseExisting(song), window, cx);
            })
            .secondary("Back", |_, _| {})
            .on_close(move |_, cx| {
                close.update(cx, |panel, cx| {
                    panel.previewed = None;
                    cx.notify();
                });
            }),
    )
}

fn render(
    panel: &NewSongPanel,
    song: SongId,
    environment: UiEnvironment,
    cx: &mut Context<NewSongPanel>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let library = panel.store.read(cx).library();
    let Ok(work) = library.song(song) else {
        return div().into_any_element();
    };
    let gap = px(scale.apply(tokens::spacing::RELATED_TIGHT));
    let fact = |selector: &'static str, label: &'static str, value: String| {
        div()
            .debug_selector(move || selector.into())
            .flex()
            .flex_col()
            .gap(gap)
            .child(
                div()
                    .text_size(px(scale.apply(tokens::typography::META.size)))
                    .text_color(rgb(palette.text_muted))
                    .child(label),
            )
            .child(div().text_color(rgb(palette.text)).child(value))
    };
    let or_dash = |text: String| {
        if text.is_empty() {
            "\u{2014}".to_owned()
        } else {
            text
        }
    };
    let aliases = work.aliases().join(", ");
    let moments = work
        .liturgical_moments()
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let arrangements = library
        .arrangements_for_song(song)
        .unwrap_or_default()
        .into_iter()
        .map(|arrangement| {
            let choirs = arrangement
                .choirs()
                .iter()
                .filter_map(|id| library.choir(*id).ok())
                .map(|choir| choir.name().to_owned())
                .collect::<Vec<_>>();
            let choirs = if choirs.is_empty() {
                "Choir unspecified".to_owned()
            } else {
                choirs.join(", ")
            };
            let languages = arrangement.languages().join(", ");
            let summary = if languages.is_empty() {
                choirs
            } else {
                format!("{languages} \u{b7} {choirs}")
            };
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(arrangement.title().to_owned()),
                )
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(summary),
                )
        })
        .collect::<Vec<_>>();
    div()
        .debug_selector(|| "new-song-preview".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .w_full()
        .child(fact(
            "new-song-preview-composer",
            "Composer / original author",
            or_dash(work.original_attribution().unwrap_or_default().to_owned()),
        ))
        .child(fact(
            "new-song-preview-aliases",
            "Aliases",
            or_dash(aliases),
        ))
        .child(fact(
            "new-song-preview-moments",
            "Usual moments",
            or_dash(moments),
        ))
        .child(
            div()
                .debug_selector(|| "new-song-preview-arrangements".into())
                .flex()
                .flex_col()
                .gap(gap)
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child("Arrangements"),
                )
                .children(arrangements),
        )
        .into_any_element()
}
