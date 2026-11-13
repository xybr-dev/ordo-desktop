//! The Arrangement details form: the mockup's fields in the mockup's order,
//! the pure functions that turn them into details and a derived label, and
//! the rendering of everything except the pickers.

use domain::{
    identity::ChoirId,
    song_library::{ArrangementDetails, SongLibrary, match_key, search_key},
};
use gpui::{Div, FontWeight, IntoElement, ParentElement, Styled, Window, div, prelude::*, px, rgb};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{button::Button, field::Field},
    tokens,
};

use super::{ArrangementDetailsPanel, Picker, pickers};

/// The labels of the six fields, in the order the form shows them.
pub(super) const FIELD_LABELS: [&str; 6] = [
    "Display title",
    "Languages",
    "Choirs",
    "Translator or adapter",
    "Occasions",
    "Source and rights notes",
];
pub(super) const NEW_TITLE: &str = "New Arrangement";
pub(super) const EDIT_TITLE: &str = "Arrangement details";
pub(super) const CREATE: &str = "Create Arrangement";
pub(super) const SAVE: &str = "Save details";
pub(super) const DUPLICATE: &str = "Duplicate Arrangement";
pub(super) const BUSY_REASON: &str = "Saving the Arrangement";
pub(super) const CHOIR_BUSY_REASON: &str = "Creating the choir";
/// Why Duplicate is blocked for an Arrangement stored with no language (a
/// parish stored before T2.6): a copy is a creation and needs one.
pub(super) const DUPLICATE_NO_LANGUAGE_REASON: &str = "Save a language before duplicating";
pub(super) const SAVED_TOAST: &str = "Saved Arrangement details.";
pub(super) const NO_LANGUAGE: &str = "Choose the language this rendition is sung in.";
pub(super) const LANGUAGE_HINT: &str = "Several only when this exact rendition mixes them.";
pub(super) const CHOIR_HINT: &str =
    "Choirs that sing this exact rendition. Different wording or order needs its own Arrangement.";
pub(super) const TRANSLATOR_HINT: &str = "The composer stays on the Song.";
pub(super) const OCCASION_HINT: &str = "Advisory, for finding it later. Leaving it empty means unspecified, not \u{201c}always\u{201d}.";
pub(super) const FAILED_CREATE: &str = "Couldn't create Arrangement";
pub(super) const FAILED_SAVE: &str = "Couldn't save details";
pub(super) const FAILED_DUPLICATE: &str = "Couldn't duplicate Arrangement";
pub(super) const FAILED_CHOIR: &str = "Couldn't create choir";

/// The occasions the form offers, in order (the mockup's list without
/// "Special"). Occasions an Arrangement already holds are offered too.
pub(super) const OCCASIONS: [&str; 5] = ["Ordinary Time", "Advent", "Christmas", "Lent", "Easter"];

/// What the Display title placeholder previews: the label the staged
/// languages and Choirs would derive. The rule is the domain's
/// (`SongLibrary::derived_label`); with no language staged the view names the
/// field instead, as the mockup does.
pub(super) fn label_preview(
    library: &SongLibrary,
    languages: &[String],
    choirs: &[ChoirId],
) -> String {
    let stand_in = ["Language".to_owned()];
    let shown: &[String] = if languages.is_empty() {
        &stand_in
    } else {
        languages
    };
    library.derived_label(shown, choirs)
}

/// The names of `choirs` in the order a label lists them: by match key of the
/// name, then identity. This is the ordering `SongLibrary::derived_label`
/// applies; the domain exposes no list of names, so the notice repeats it.
pub(super) fn choir_names_in_label_order(library: &SongLibrary, choirs: &[ChoirId]) -> Vec<String> {
    let mut named: Vec<(String, ChoirId, String)> = choirs
        .iter()
        .filter_map(|id| library.choir(*id).ok())
        .map(|choir| (match_key(choir.name()), choir.id(), choir.name().to_owned()))
        .collect();
    named.sort();
    named.into_iter().map(|(_, _, name)| name).collect()
}

/// "A", "A and B" or "A, B and C".
pub(super) fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The details the fields stage. A blank text is no value; an authored one is
/// kept verbatim.
pub fn build_details(
    title: &str,
    languages: &[String],
    choirs: &[ChoirId],
    translator: &str,
    occasions: &[String],
    notes: &str,
) -> ArrangementDetails {
    let some = |text: &str| Some(text.to_owned()).filter(|text| !text.trim().is_empty());
    ArrangementDetails {
        title: some(title),
        languages: languages.to_vec(),
        choirs: choirs.to_vec(),
        rendition_attribution: some(translator),
        occasions: occasions.to_vec(),
        notes: some(notes),
    }
}

/// Whether two language names are the same language, ignoring case and
/// diacritics.
pub(super) fn same_language(a: &str, b: &str) -> bool {
    search_key(a) == search_key(b)
}

pub(super) fn render(
    panel: &mut ArrangementDetailsPanel,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut gpui::Context<ArrangementDetailsPanel>,
) -> impl IntoElement + use<> {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let surface = palette.surface;
    let meta = move |text: &'static str| {
        div()
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .text_color(rgb(palette.text_muted))
            .child(text)
    };
    let stack = move || {
        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
    };
    let label = move |text: &'static str, hint: Option<&'static str>| {
        div()
            .flex()
            .items_baseline()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
            .text_color(rgb(palette.text_muted))
            .child(text)
            .children(hint.map(|hint| {
                div()
                    .text_size(px(scale.apply(tokens::typography::META.size)))
                    .child(hint)
            }))
    };
    let icon_of = move |path: &'static str, color: u32| {
        gpui::svg()
            .path(path)
            .flex_none()
            .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
            .text_color(rgb(color))
    };

    let placeholder = {
        let library = panel.store.read(cx).library();
        format!(
            "Leave blank to show {}",
            label_preview(library, &panel.languages, &panel.choirs)
        )
    };
    let notice = panel.shared_notice(cx).map(|names| {
        div()
            .debug_selector(|| "ad-shared-notice".into())
            .flex()
            .w_full()
            .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .bg(rgb(palette.surface_secondary))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("Shared by {}.", join_names(&names))),
                    )
                    .child(
                        "Saving updates this one Arrangement for every choir that uses it. Mass plans keep their references; past Presentation snapshots don\u{2019}t change.",
                    ),
            )
    });

    let languages = {
        let chips = pickers::language_chips(panel, environment, cx);
        let language_error = panel.tried && panel.languages.is_empty();
        let tail = if language_error {
            div()
                .debug_selector(|| "ad-language-error".into())
                .flex()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.danger))
                .child(icon_of(icon::CIRCLE_ALERT, palette.danger))
                .child(NO_LANGUAGE)
        } else {
            div()
                .debug_selector(|| "ad-language-hint".into())
                .child(meta(LANGUAGE_HINT))
        };
        let picker = pickers::picker(panel, Picker::Language, environment, window, cx);
        div().debug_selector(|| "ad-languages".into()).child(
            stack()
                .child(label(FIELD_LABELS[1], Some("at least one")))
                .child(chips)
                .children(picker)
                .child(tail),
        )
    };

    let choirs = {
        let chips = pickers::choir_chips(panel, environment, cx);
        let tail = match &panel.made {
            Some(name) => div()
                .debug_selector(|| "ad-choir-made".into())
                .flex()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child(icon_of(icon::CHECK, palette.text_muted))
                .child(format!(
                    "Created the choir {name}. Add its short label and preferred languages later in Library \u{2192} Choirs."
                )),
            None => div().child(meta(CHOIR_HINT)),
        };
        let picker = pickers::picker(panel, Picker::Choir, environment, window, cx);
        div().debug_selector(|| "ad-choirs".into()).child(
            stack()
                .child(label(FIELD_LABELS[2], None))
                .child(chips)
                .children(picker)
                .child(tail),
        )
    };

    let occasions = {
        let chips = pickers::occasion_chips(panel, environment, cx);
        div().debug_selector(|| "ad-occasions".into()).child(
            stack()
                .child(label(FIELD_LABELS[4], None))
                .child(chips)
                .child(meta(OCCASION_HINT)),
        )
    };

    let wrap = |selector: &'static str, field: Field| {
        div().debug_selector(move || selector.into()).child(field)
    };
    let failure = panel.error.clone().map(|(heading, reason)| {
        div()
            .debug_selector(|| "ad-error".into())
            .flex()
            .flex_col()
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .text_color(rgb(palette.danger))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(heading))
            .child(reason)
    });

    let weak = cx.weak_entity();
    let root = div().debug_selector(|| "ad-form".into());
    // Escape closes an open picker before it closes the panel.
    ui::overlay::escape_first(root, move |window, cx| {
        weak.update(cx, |panel, cx| {
            let open = panel.picker.is_some();
            panel.close_picker(window, cx);
            open
        })
        .unwrap_or(false)
    })
    .flex()
    .flex_col()
    .gap(px(scale.apply(tokens::spacing::GROUP)))
    .w_full()
    .capture_key_down(
        cx.listener(|panel, event: &gpui::KeyDownEvent, window, cx| {
            panel.key_down(event, window, cx);
        }),
    )
    .children(notice)
    .child(wrap(
        "ad-title",
        Field::new("ad-title-field", FIELD_LABELS[0], &panel.title, environment)
            .placeholder(placeholder)
            .on_surface(surface),
    ))
    .child(languages)
    .child(choirs)
    .child(
        stack()
            .child(wrap(
                "ad-translator",
                Field::new(
                    "ad-translator-field",
                    FIELD_LABELS[3],
                    &panel.translator,
                    environment,
                )
                .placeholder("Optional")
                .on_surface(surface),
            ))
            .child(meta(TRANSLATOR_HINT)),
    )
    .child(occasions)
    .child(wrap(
        "ad-notes",
        Field::new("ad-notes-field", FIELD_LABELS[5], &panel.notes, environment)
            .placeholder("Optional")
            .on_surface(surface),
    ))
    .children(failure)
}

/// A chip: a toggle that shows a check while it is on.
pub(super) fn chip(
    id: impl Into<gpui::ElementId>,
    text: String,
    on: bool,
    environment: UiEnvironment,
    activate: impl Fn(&mut Window, &mut gpui::App) + 'static,
) -> Button {
    let palette = tokens::palette(environment.appearance());
    let mut chip = Button::new(id, text, environment)
        .current(on)
        .on_surface(palette.surface)
        .on_activate(activate);
    if on {
        chip = chip.icon(icon::CHECK);
    }
    chip
}

/// The wrapping row chips sit in.
pub(super) fn chip_row(environment: UiEnvironment) -> Div {
    let scale = environment.preferences().scale;
    div()
        .flex()
        .flex_wrap()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
}
