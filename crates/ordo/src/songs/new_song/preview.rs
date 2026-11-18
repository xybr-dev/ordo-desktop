//! The nested preview of an existing Song: identity and a compact Arrangement
//! summary only. No lyrics and nothing to edit.

use domain::identity::SongId;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px, rgb,
};
use gpui::Window;
use ui::{environment::UiEnvironment, overlay::Overlay, tokens};

use super::{NewSongPanel, Outcome};
use crate::songs::store::LibraryStore;

pub(super) const DESCRIPTION: &str = "Existing Song \u{b7} preview only";

/// The preview slideover for `song`, or `None` when the library no longer has
/// it.
pub(super) fn overlay(
    panel: &NewSongPanel,
    song: SongId,
    cx: &mut Context<NewSongPanel>,
) -> Option<Overlay> {
    let close = cx.entity();
    let report = panel.report.clone();
    song_overlay(
        panel.store.clone(),
        song,
        true,
        move |window, cx| report(Outcome::UseExisting(song), window, cx),
        move |_, cx| {
            close.update(cx, |panel, cx| {
                panel.previewed = None;
                cx.notify();
            });
        },
        cx,
    )
}

/// The nested preview of an existing Song, shared by every flow that offers
/// one (New Song, the Markdown draft): identity and an Arrangement summary,
/// **Back**, and **Use this existing Song** running `use_existing`. When
/// `attach_to_archived` is false an archived Song cannot take an Arrangement,
/// so its preview offers Back only.
/// `on_close` runs once the surface has gone, however it was dismissed. `None`
/// when the Library no longer has the Song.
pub(crate) fn song_overlay(
    store: Entity<LibraryStore>,
    song: SongId,
    attach_to_archived: bool,
    use_existing: impl Fn(&mut Window, &mut App) + 'static,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> Option<Overlay> {
    let (title, archived) = {
        let library = store.read(cx).library();
        let work = library.song(song).ok()?;
        (
            work.title().to_owned(),
            work.is_archived() && !attach_to_archived,
        )
    };
    let body = store.clone();
    let gone = store;
    let overlay = Overlay::slideover(title.clone())
        .description(if archived {
            format!("{title} is archived. Restore it to attach to it.")
        } else {
            DESCRIPTION.to_owned()
        })
        .body(move |environment, _, cx| render(&body, song, environment, cx))
        .secondary("Back", |_, _| {})
        .on_close(move |window, cx| on_close(window, cx));
    Some(if archived {
        overlay
    } else {
        overlay.primary("Use this existing Song", move |window, cx| {
            if gone.read(cx).library().song(song).is_ok() {
                use_existing(window, cx);
            }
        })
    })
}

fn render(
    store: &Entity<LibraryStore>,
    song: SongId,
    environment: UiEnvironment,
    cx: &mut App,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let Some(facts) = Facts::of(store.read(cx).library(), song) else {
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
    let arrangements = facts
        .arrangements
        .iter()
        .map(|(title, summary)| {
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(title.clone()),
                )
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(summary.clone()),
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
            or_dash(facts.composer.clone()),
        ))
        .child(fact(
            "new-song-preview-aliases",
            "Aliases",
            or_dash(facts.aliases.clone()),
        ))
        .child(fact(
            "new-song-preview-moments",
            "Usual moments",
            or_dash(facts.moments.clone()),
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

/// Every piece of Song text the preview shows: identity and an Arrangement
/// summary (title, then languages and Choirs). Lyrics are not among them.
pub(super) struct Facts {
    pub composer: String,
    pub aliases: String,
    pub moments: String,
    pub arrangements: Vec<(String, String)>,
}

impl Facts {
    pub(super) fn of(library: &domain::song_library::SongLibrary, song: SongId) -> Option<Self> {
        let work = library.song(song).ok()?;
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
                (
                    library
                        .arrangement_label(arrangement.id())
                        .unwrap_or_default(),
                    summary,
                )
            })
            .collect();
        Some(Self {
            composer: work.original_attribution().unwrap_or_default().to_owned(),
            aliases: work.aliases().join(", "),
            moments: work
                .liturgical_moments()
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            arrangements,
        })
    }

    /// All of the text above in one string, for a test to search.
    #[cfg(test)]
    pub(super) fn text(&self) -> String {
        let mut text = vec![
            self.composer.clone(),
            self.aliases.clone(),
            self.moments.clone(),
        ];
        for (title, summary) in &self.arrangements {
            text.push(title.clone());
            text.push(summary.clone());
        }
        text.join("\n")
    }
}