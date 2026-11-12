//! The New song form: the mockup's four fields, the draft they stage, and the
//! collapsed possible-existing-Songs block under them.

use domain::song_library::SongDraft;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, KeyDownEvent, ParentElement, Styled, Window, div,
    prelude::*, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    primitives::{
        disclosure::Disclosure,
        field::Field,
        list::{Badge, ListRow},
    },
    tokens,
};

use super::NewSongPanel;

pub(super) const TITLE: &str = "New song";
pub(super) const DESCRIPTION: &str =
    "The composition itself. Lyrics, language and choirs belong to its Arrangements.";
/// The identity fields the form renders, in order. Nothing else is a field.
pub(super) const FIELD_LABELS: [&str; 4] = [
    "Primary title",
    "Composer or original author",
    "Aliases",
    "Usual moment",
];
pub(super) const CREATE: &str = "Create song";
pub(super) const CREATE_REASON: &str = "Type a primary title first";
pub(super) const FAILED: &str = "Couldn't create song";

/// The draft the four fields stage. The title is kept verbatim; aliases and
/// moments are split on commas, trimmed at the ends and stripped of empties;
/// a blank composer is no composer.
pub fn build_draft(title: &str, composer: &str, aliases: &str, moments: &str) -> SongDraft {
    let list = |text: &str| -> Vec<String> {
        text.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let mut draft = SongDraft::titled(title);
    draft.original_attribution =
        Some(composer.to_owned()).filter(|composer| !composer.trim().is_empty());
    draft.aliases = list(aliases);
    draft.liturgical_moments = list(moments);
    draft
}

/// "1 possible existing Song" or "N possible existing Songs".
pub(super) fn count_label(count: usize) -> String {
    format!(
        "{count} possible existing Song{}",
        if count == 1 { "" } else { "s" }
    )
}

/// "1 Arrangement" or "N Arrangements".
pub(super) fn arrangement_count(count: usize) -> String {
    format!("{count} Arrangement{}", if count == 1 { "" } else { "s" })
}

pub(super) fn render(
    panel: &mut NewSongPanel,
    environment: UiEnvironment,
    _window: &mut Window,
    cx: &mut Context<NewSongPanel>,
) -> impl IntoElement + use<> {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let surface = palette.surface;
    let field = |selector: &'static str, field: Field| {
        div().debug_selector(move || selector.into()).child(field)
    };

    let candidates = candidates(panel, environment, cx);
    let error = panel.error.clone().map(|reason| {
        div()
            .debug_selector(|| "new-song-error".into())
            .flex()
            .flex_col()
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .text_color(rgb(palette.danger))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(FAILED))
            .child(reason)
    });

    div()
        .debug_selector(|| "new-song-form".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .w_full()
        .on_key_down(cx.listener(|panel, event: &KeyDownEvent, window, cx| {
            // Enter in the title does what Create song does.
            if event.keystroke.key == "enter"
                && !event.keystroke.modifiers.modified()
                && panel.title.focus_handle(cx).is_focused(window)
            {
                panel.submit(window, cx);
                cx.stop_propagation();
            }
        }))
        .child(field(
            "new-song-title",
            Field::new(
                "new-song-title-field",
                FIELD_LABELS[0],
                &panel.title,
                environment,
            )
            .placeholder("Ginoo, Kaloy-i Kami")
            .on_surface(surface),
        ))
        .child(candidates)
        .child(field(
            "new-song-composer",
            Field::new(
                "new-song-composer-field",
                FIELD_LABELS[1],
                &panel.composer,
                environment,
            )
            .placeholder("Optional")
            .on_surface(surface),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(field(
                    "new-song-aliases",
                    Field::new(
                        "new-song-aliases-field",
                        FIELD_LABELS[2],
                        &panel.aliases,
                        environment,
                    )
                    .placeholder("Optional, separated by commas")
                    .on_surface(surface),
                ))
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child("Other names people search for, such as Kyrie."),
                ),
        )
        .child(field(
            "new-song-moment",
            Field::new(
                "new-song-moment-field",
                FIELD_LABELS[3],
                &panel.moments,
                environment,
            )
            .placeholder("Optional, such as Entrance or Communion")
            .on_surface(surface),
        ))
        .children(error)
}

/// The collapsed count of possible existing Songs and, when expanded, their
/// rows. Hidden while there are none; never opened or chosen automatically.
fn candidates(
    panel: &mut NewSongPanel,
    environment: UiEnvironment,
    cx: &mut Context<NewSongPanel>,
) -> AnyElement {
    if panel.candidates.is_empty() {
        return div().into_any_element();
    }
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let library = panel.store.read(cx).library();
    let weak = cx.weak_entity();
    let rows = panel
        .candidates
        .iter()
        .enumerate()
        .filter_map(|(at, possible)| {
            let song = library.song(possible.song).ok()?;
            let arrangements = library
                .arrangements_for_song(possible.song)
                .map_or(0, |all| all.len());
            let detail = match song.original_attribution() {
                Some(composer) => {
                    format!("{composer} \u{b7} {}", arrangement_count(arrangements))
                }
                None => arrangement_count(arrangements),
            };
            let id = possible.song;
            let weak = weak.clone();
            let row = ListRow::new(
                ("new-song-candidate-row", at),
                song.title().to_owned(),
                environment,
            )
            .detail(detail)
            .badges(
                song.is_archived()
                    .then(|| Badge::new("Archived", environment)),
            )
            .on_surface(palette.surface)
            .on_activate(move |window, cx| {
                let _ = weak.update(cx, |panel, cx| panel.preview(id, window, cx));
            });
            Some(
                div()
                    .debug_selector(move || format!("new-song-candidate-{at}"))
                    .w_full()
                    .child(row),
            )
        })
        .collect::<Vec<_>>();
    let list = div()
        .debug_selector(|| "new-song-candidates".into())
        .flex()
        .flex_col()
        .w_full()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .children(rows);
    div()
        .debug_selector(|| "new-song-possible".into())
        .child(
            Disclosure::new(
                "new-song-possible-disclosure",
                count_label(panel.candidates.len()),
                &panel.disclosure,
                environment,
                list,
            )
            .on_surface(palette.surface),
        )
        .into_any_element()
}
