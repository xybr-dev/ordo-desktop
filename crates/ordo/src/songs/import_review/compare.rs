//! Compare contents: the read-only comparison of one imported Arrangement
//! with its nearest local candidate, opened over the review as a wide
//! slideover. It reuses History's comparison body with import headings; it
//! holds no field and no edit control, and nothing it does reaches the
//! review's staged action, selection or scroll.

use std::collections::HashMap;

use domain::{
    identity::{ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId},
    song_library::{
        ArrangementDetails, ArrangementLyrics, OccurrenceDraft, PaginationState, SectionDraft,
        SongLibrary,
        import::{ImportDocument, SequenceEntry, SongChoice, SongIdentity, Staged, align_sections},
        match_key,
    },
};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement, Styled, Window,
    div, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    overlay::{Overlay, OverlayHost},
    primitives::selection::{Choice, Select, SelectionState},
    tokens,
};

use super::review::ImportReview;
use crate::{
    history::{
        compare::{ArrangementSide, ColumnTitles, CompareView, Sides},
        slides::SlideFrames,
    },
    songs::store::LibraryStore,
};

/// The surface's title.
pub const TITLE: &str = "Compare contents";

/// The line under the title (mockup 887).
const META: &str = "Read-only \u{b7} the staged action doesn\u{2019}t change here";

/// The footer's one action (mockup 891).
const BACK_TO_REVIEW: &str = "Back to review";

/// The Library column's heading when nothing matches.
const LIBRARY: &str = "Library";

/// A source of identities that exist only for this view: they never reach the
/// Library, and a random-looking prefix keeps them off real ones.
struct Synthetic(u64);

impl Synthetic {
    fn next(&mut self, tag: u8) -> EntityId {
        self.0 += 1;
        let mut bytes = [tag; 16];
        bytes[8..].copy_from_slice(&self.0.to_le_bytes());
        EntityId::from_random_bytes(bytes)
    }
}

/// The imported Arrangement as a comparison side, its section ids aligned to
/// the local side's for display (Decision 19): a section takes the id of the
/// local section with the same label, then by position among the unlabeled, so
/// changed lines are marked rather than whole sections. Imported sections are
/// NeedsReview, as a commit would write them. Imported Choir names with no
/// local Choir are added to `choirs` under identities local to the view.
pub fn imported_side(
    document: &ImportDocument,
    arrangement: usize,
    local: Option<&domain::song_library::Arrangement>,
    choirs: &mut HashMap<ChoirId, String>,
) -> ArrangementSide {
    let empty = ArrangementSide {
        details: ArrangementDetails::default(),
        lyrics: ArrangementLyrics {
            sections: Vec::new(),
            sequence: Vec::new(),
        },
        recorded: true,
    };
    let Some(imported) = document.arrangements.get(arrangement) else {
        return empty;
    };
    let mut synthetic = Synthetic(0);
    let mut mint = || synthetic.next(0xC0);
    let ids: Vec<SectionId> = match local {
        Some(local) => align_sections(&imported.sections, local.sections(), &mut mint),
        None => imported
            .sections
            .iter()
            .map(|_| SectionId::from_entity(mint()))
            .collect(),
    };
    let sections: Vec<SectionDraft> = imported
        .sections
        .iter()
        .zip(&ids)
        .map(|(section, id)| {
            let draft = match &section.label {
                Some(label) => SectionDraft::labeled(*id, label.clone(), section.lyrics.clone()),
                None => SectionDraft::new(*id, section.lyrics.clone()),
            };
            draft.with_pagination(PaginationState::NeedsReview)
        })
        .collect();
    let mut occurrences = Synthetic(0);
    let sequence: Vec<OccurrenceDraft> = imported
        .sequence
        .iter()
        .filter_map(|entry| match entry {
            SequenceEntry::Section(at) => ids.get(*at).copied(),
            SequenceEntry::Unresolved { .. } => None,
        })
        .map(|section| {
            OccurrenceDraft::new(OccurrenceId::from_entity(occurrences.next(0xC2)), section)
        })
        .collect();
    let mut linked: Vec<ChoirId> = Vec::new();
    let mut names = Synthetic(0);
    for name in &imported.choirs {
        let key = match_key(name);
        let known = choirs
            .iter()
            .find(|(_, known)| match_key(known) == key)
            .map(|(id, _)| *id);
        let id = known.unwrap_or_else(|| {
            let mut id = ChoirId::from_entity(names.next(0xC1));
            while choirs.contains_key(&id) {
                id = ChoirId::from_entity(names.next(0xC1));
            }
            choirs.insert(id, name.clone());
            id
        });
        if !linked.contains(&id) {
            linked.push(id);
        }
    }
    ArrangementSide {
        details: ArrangementDetails {
            title: imported.title.clone(),
            languages: imported.languages.clone(),
            choirs: linked,
            rendition_attribution: imported.rendition_attribution.clone(),
            occasions: imported.occasions.clone(),
            notes: imported.notes.clone(),
        },
        lyrics: ArrangementLyrics { sections, sequence },
        recorded: true,
    }
}

/// The local Arrangement as the right column's side.
fn local_side(library: &SongLibrary, id: ArrangementId) -> Option<ArrangementSide> {
    let arrangement = library.arrangement(id).ok()?;
    Some(ArrangementSide {
        details: arrangement.details(),
        lyrics: arrangement.lyrics(),
        recorded: true,
    })
}

/// The comparison surface's state, held by its overlay and by the review that
/// opened it.
pub struct ImportCompare {
    view: Entity<CompareView>,
    document: ImportDocument,
    arrangement: usize,
    store: Entity<LibraryStore>,
    /// The local Arrangements the right column can show, in the Select's
    /// order; empty when the Song has none.
    options: Vec<(ArrangementId, String)>,
    nearest: SelectionState,
    choirs: HashMap<ChoirId, String>,
}

impl ImportCompare {
    /// The comparison body.
    pub fn view(&self) -> &Entity<CompareView> {
        &self.view
    }

    /// The Library Arrangements the **Nearest match** select offers, by label;
    /// the select is drawn only when there are more than one.
    pub fn candidates(&self) -> Vec<String> {
        self.options
            .iter()
            .map(|(_, label)| label.clone())
            .collect()
    }

    /// The Arrangement the right column shows now.
    pub fn nearest(&self, cx: &App) -> Option<ArrangementId> {
        self.nearest
            .selected(cx)
            .and_then(|at| self.options.get(at))
            .map(|(id, _)| *id)
    }

    /// Shows the candidate at `index` of the **Nearest match** select: the
    /// right column is rebuilt and the imported side is aligned to it again.
    /// The review is not touched.
    pub fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((id, _)) = self.options.get(index).cloned() else {
            return;
        };
        self.nearest.select(index, cx);
        self.show(Some(id), cx);
    }

    fn show(&mut self, local: Option<ArrangementId>, cx: &mut Context<Self>) {
        let (left, right, title) = {
            let library = self.store.read(cx).library();
            let arrangement = local.and_then(|id| library.arrangement(id).ok());
            let left = imported_side(
                &self.document,
                self.arrangement,
                arrangement,
                &mut self.choirs,
            );
            let right = local.and_then(|id| local_side(library, id));
            let title = match local.and_then(|id| library.arrangement_label(id).ok()) {
                Some(label) => format!("{LIBRARY} \u{b7} {label}"),
                None => LIBRARY.to_owned(),
            };
            (left, right, title)
        };
        let choirs = self.choirs.clone();
        self.view.update(cx, |view, cx| {
            view.set_choirs(choirs);
            view.replace_sides(left, right, title, cx);
        });
        cx.notify();
    }

    fn render_body(
        &mut self,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let weak = cx.weak_entity();
        let select = (self.options.len() > 1).then(|| {
            div()
                .flex()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child("Nearest match"),
                )
                .child(
                    Select::new(
                        "import-compare-nearest",
                        "Nearest match",
                        &self.nearest,
                        environment,
                    )
                    .on_select(move |choice, _, cx| {
                        if let Some(choice) = choice {
                            let _ = weak.update(cx, |compare, cx| compare.choose(choice, cx));
                        }
                    }),
                )
        });
        let body = self
            .view
            .update(cx, |view, cx| view.render_body(environment, window, cx));
        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .children(select)
            .child(body)
            .into_any_element()
    }
}

/// Opens the read-only comparison of one imported Arrangement and its nearest
/// local candidate over the review. `closed` runs once however the surface is
/// dismissed. Returns the surface's state and the id the host gave it, so the
/// opener can close it by id.
pub fn open_compare(
    review: &ImportReview,
    document: usize,
    arrangement: usize,
    host: OverlayHost,
    closed: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<(usize, Entity<ImportCompare>)> {
    let source = review.documents().get(document)?;
    let Staged::Valid(imported) = &source.content else {
        return None;
    };
    let file = source.source.clone();
    let imported = imported.clone();
    let reviewed = review.review_of(document)?;
    let song = match review.choices().songs.get(&document) {
        Some(SongChoice::Existing(song)) => Some(*song),
        Some(_) => None,
        None => match &reviewed.identity {
            SongIdentity::Existing { song, .. } => Some(*song),
            _ => None,
        },
    };
    let nearest = reviewed.arrangements.get(arrangement)?.nearest;
    let store = review.store.clone();
    let (options, mut choirs) = {
        let library = store.read(cx).library();
        let options: Vec<(ArrangementId, String)> = song
            .and_then(|song| library.arrangements_for_song(song).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|local| {
                let id = local.id();
                (id, library.arrangement_label(id).unwrap_or_default())
            })
            .collect();
        let choirs: HashMap<ChoirId, String> = library
            .choirs()
            .map(|choir| (choir.id(), choir.name().to_owned()))
            .collect();
        (options, choirs)
    };
    let initial = nearest
        .and_then(|id| options.iter().position(|(option, _)| *option == id))
        .or_else(|| (!options.is_empty()).then_some(0));
    let choices: Vec<Choice> = options
        .iter()
        .map(|(_, label)| Choice::new(label.clone()))
        .collect();
    let nearest_state = SelectionState::new(choices, initial, cx);
    let local_id = initial.and_then(|at| options.get(at)).map(|(id, _)| *id);
    let left = {
        let library = store.read(cx).library();
        let local = local_id.and_then(|id| library.arrangement(id).ok());
        imported_side(&imported, arrangement, local, &mut choirs)
    };
    let right = {
        let library = store.read(cx).library();
        local_id.and_then(|id| local_side(library, id))
    };
    let right_title = {
        let library = store.read(cx).library();
        match local_id.and_then(|id| library.arrangement_label(id).ok()) {
            Some(label) => format!("{LIBRARY} \u{b7} {label}"),
            None => LIBRARY.to_owned(),
        }
    };
    let none = right.is_none();
    let frames = cx.new(|_| SlideFrames::new());
    let view = CompareView::new(
        Sides::Arrangement {
            current: left.clone(),
            revision: right.clone().unwrap_or_else(|| left.clone()),
        },
        choirs.clone(),
        ColumnTitles {
            left: format!("Imported \u{b7} {file}"),
            right: right_title.clone(),
        },
        (String::new(), String::new()),
        frames,
        cx,
    );
    if none {
        view.update(cx, |view, cx| {
            view.replace_sides(left, None, right_title, cx)
        });
    }
    let compare = cx.new(|_| ImportCompare {
        view,
        document: imported,
        arrangement,
        store,
        options,
        nearest: nearest_state,
        choirs,
    });
    let body = compare.clone();
    let overlay = Overlay::slideover(TITLE)
        .wide()
        .description(META)
        .body(move |environment, window, cx| {
            body.update(cx, |compare, cx| {
                compare.render_body(environment, window, cx)
            })
        })
        .secondary(BACK_TO_REVIEW, |_, _| {})
        .on_close(closed);
    let id = host.open(overlay, window, cx);
    Some((id, compare))
}
