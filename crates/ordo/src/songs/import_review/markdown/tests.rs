//! Markdown draft workspace behaviour tests, over a real parish store in a
//! temporary directory, a real window, and real clicks and keys.

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use domain::{
    identity::{ArrangementId, EntityId, SongId},
    song_library::{
        ReadinessState, SongDraft,
        import::{SongChoice, markdown},
        readiness,
    },
};
use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window,
};
use ui::{
    environment::{ResolvedAppearance, UiEnvironment, UiPreferences},
    overlay::OverlayHost,
};

use super::{MarkdownDraftEvent, MarkdownDraftView, issues, preview};
use crate::songs::{
    LibraryStore, StorageSlot,
    choirs::tests::{click, open_parish, temp_dir, wait},
};

const FILE: &str = "gloria.md";
const GLORIA: &str = "---\ntitle: Gloria\nlanguage: English\n---\n[Verse]\nGlory to God\n";
/// An unknown key on line 4 and an unresolved repeat on line 8.
const NOISY: &str = "---\ntitle: Gloria\nlanguage: English\ncolour: blue\n---\n[Verse]\nGlory to God\n[Repeat: Coda]\n";
const NO_LANGUAGE: &str = "---\ntitle: Gloria\n---\n[Verse]\nGlory to God\n";
const REPEATS: &str = "---\ntitle: Repeats\nlanguage: English\n---\n[Verse 1]\nFirst line\n\nsecond slide of verse\n\n[Repeat: chorus]\n\n[Chorus]\nSing it out\n[Repeat:  VERSE 1 ]\n";
const TINAPAY: &str =
    "---\ntitle: Tinapay sa Langit\nlanguage: English\n---\n[Verse]\nBread from heaven\n";

fn song_id(byte: u8) -> SongId {
    SongId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

fn arrangement_id(byte: u8) -> ArrangementId {
    ArrangementId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

/// Hosts the draft like the shell does: draws its frame and the overlay layer.
struct Host {
    view: Entity<MarkdownDraftView>,
    overlays: OverlayHost,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
        let frame = self
            .view
            .update(cx, |view, cx| view.render_frame(environment, window, cx));
        gpui::div()
            .size_full()
            .child(frame)
            .child(self.overlays.layer(environment, window, cx))
    }
}

struct Rig {
    view: Entity<MarkdownDraftView>,
    store: Entity<LibraryStore>,
    slot: StorageSlot,
    dir: std::path::PathBuf,
    events: Rc<RefCell<Vec<MarkdownDraftEvent>>>,
    overlays: OverlayHost,
    /// The Songs seeded before the draft opened.
    songs: Vec<SongId>,
}

impl Rig {
    fn library_songs(&self, cx: &mut VisualTestContext) -> usize {
        self.store
            .read_with(cx, |store, _| store.library().songs().count())
    }

    fn arrangements_of(&self, cx: &mut VisualTestContext, song: SongId) -> usize {
        self.store.read_with(cx, |store, _| {
            store
                .library()
                .arrangements_for_song(song)
                .map_or(0, |list| list.len())
        })
    }

    fn text(&self, cx: &mut VisualTestContext) -> String {
        self.view
            .read_with(cx, |view, cx| view.source().value(cx).to_string())
    }

    fn depth(&self, cx: &mut VisualTestContext) -> usize {
        cx.update(|_, cx| self.overlays.depth(cx))
    }

    fn finish(&self, cx: &mut VisualTestContext) {
        let outcome = cx.foreground_executor().block_test(self.slot.shutdown());
        assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Opens the draft of `text` over a parish already holding Songs titled
/// `seed`.
fn open<'a>(
    cx: &'a mut TestAppContext,
    tag: &str,
    seed: &[&str],
    text: &str,
) -> (Rig, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    let dir = temp_dir(tag);
    let (slot, store) = open_parish(cx, &dir);
    let mut songs = Vec::new();
    for title in seed {
        let task = store.update(cx, |store, cx| {
            store.create_song(SongDraft::titled(*title), cx)
        });
        songs.push(cx.foreground_executor().block_test(task).expect("seeded"));
    }
    cx.update(crate::init_ui).expect("ui");
    let bytes = text.as_bytes().to_vec();
    let draft = markdown::parse(&bytes, (song_id(0xa1), arrangement_id(0xa2)));
    let events = Rc::new(RefCell::new(Vec::new()));
    let for_window = (store.clone(), events.clone());
    let (host, cx) = cx.add_window_view(move |window, cx| {
        let (store, events) = for_window;
        let overlays = OverlayHost::new(cx);
        let view =
            cx.new(|cx| MarkdownDraftView::new(FILE.to_owned(), bytes, draft, store, window, cx));
        view.update(cx, |view, _| view.set_overlays(overlays.clone()));
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(&view, window, move |_, _, event, _, _| {
            events.borrow_mut().push(*event);
        })
        .detach();
        Host { view, overlays }
    });
    cx.update(|window, _| window.activate_window());
    cx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    cx.run_until_parked();
    let (view, overlays) = host.read_with(cx, |host, _| (host.view.clone(), host.overlays.clone()));
    (
        Rig {
            view,
            store,
            slot,
            dir,
            events,
            overlays,
            songs,
        },
        cx,
    )
}

fn present(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// Lets an overlay slide finish so its controls are where they will stay.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..10 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(60));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

/// A click, then the motion it started finished.
fn tap(cx: &mut VisualTestContext, selector: &'static str) {
    click(cx, selector);
    settle(cx);
}

/// Selects `range` of the source text, focuses it, and types `text` over it.
fn type_over(rig: &Rig, cx: &mut VisualTestContext, range: std::ops::Range<usize>, text: &str) {
    let source = rig.view.read_with(cx, |view, _| view.source().clone());
    cx.update(|window, cx| {
        source.set_selected_range(range, cx);
        source.focus(window, cx);
    });
    if text.is_empty() {
        cx.simulate_keystrokes("backspace");
    } else {
        cx.simulate_input(text);
    }
    cx.run_until_parked();
}

/// Waits for the re-parse of the latest text to land.
fn parsed(rig: &Rig, cx: &mut VisualTestContext) {
    let view = rig.view.clone();
    wait(cx, |cx| !view.read_with(cx, |view, _| view.parsing()));
}

fn rows(rig: &Rig, cx: &mut VisualTestContext) -> Vec<String> {
    rig.view.read_with(cx, |view, _| {
        view.parsed().issues.iter().map(issues::row_text).collect()
    })
}

fn sequence(rig: &Rig, cx: &mut VisualTestContext) -> Vec<String> {
    rig.view.read_with(cx, |view, _| {
        view.parsed()
            .document
            .arrangements
            .first()
            .map(preview::sequence_lines)
            .unwrap_or_default()
    })
}

#[gpui::test]
fn ac3_a_markdown_file_opens_directly_as_an_unsaved_draft(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-open", &[], GLORIA);
    rig.view.read_with(cx, |view, cx| {
        assert_eq!(view.parsed().document.song.title, "Gloria");
        assert!(view.can_save());
        assert_eq!(view.source().value(cx).as_ref(), GLORIA);
        assert!(!view.has_edits(cx));
        assert_eq!(view.target(), SongChoice::New);
    });
    for selector in [
        "markdown-title",
        "markdown-source",
        "markdown-issues",
        "markdown-details",
        "markdown-preview",
        "markdown-section-0",
        "markdown-sequence-0",
        "button-Save song",
        "button-Discard draft",
    ] {
        assert!(present(cx, selector), "{selector}");
    }
    let focused = cx.update(|window, cx| {
        rig.view
            .read(cx)
            .source()
            .focus_handle(cx)
            .is_focused(window)
    });
    assert!(focused, "focus starts in the source text");
    assert_eq!(rig.library_songs(cx), 0, "nothing is saved yet");
    rig.finish(cx);
}

#[gpui::test]
fn ac3_inline_issues_name_their_line_and_move_the_caret(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-issues", &[], NOISY);
    assert_eq!(
        rows(&rig, cx),
        [
            "Line 4 \u{b7} Ordo doesn't use colour; it's ignored.",
            "Line 8 \u{b7} No section is called Coda."
        ]
    );
    let caret =
        |cx: &mut VisualTestContext| rig.view.read_with(cx, |view, cx| view.source().cursor(cx));
    // A click moves the caret to the start of the line and focuses the text.
    click(cx, "markdown-issue-1");
    assert_eq!(caret(cx), NOISY.find("[Repeat: Coda]").expect("line 8"));
    let focused = cx.update(|window, cx| {
        rig.view
            .read(cx)
            .source()
            .focus_handle(cx)
            .is_focused(window)
    });
    assert!(focused);
    // Enter does the same from the keyboard: Tab reaches the first row.
    type_over(&rig, cx, 0..0, "");
    assert_eq!(caret(cx), 0);
    cx.simulate_keystrokes("tab enter");
    cx.run_until_parked();
    assert_eq!(caret(cx), NOISY.find("colour").expect("line 4"));
    rig.finish(cx);
}

#[gpui::test]
fn ac3_a_blocking_issue_disables_save(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-blocking", &[], NO_LANGUAGE);
    assert_eq!(
        rows(&rig, cx),
        ["Line 1 \u{b7} Add a language: one of Ordo's languages."]
    );
    assert!(
        rig.view
            .read_with(cx, |view, _| view.parsed().issues[0].blocking)
    );
    assert!(!rig.view.read_with(cx, |view, _| view.can_save()));
    click(cx, "button-Save song");
    assert!(
        rig.events.borrow().is_empty(),
        "a disabled Save does nothing"
    );
    assert_eq!(rig.library_songs(cx), 0);
    // Writing the language lifts the block, and Save then works.
    let at = NO_LANGUAGE.find("\n---\n[Verse]").expect("end of title");
    type_over(&rig, cx, at..at, "");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("language: English");
    parsed(&rig, cx);
    assert!(rig.view.read_with(cx, |view, _| view.can_save()));
    assert!(rows(&rig, cx).is_empty());
    click(cx, "button-Save song");
    let events = rig.events.clone();
    wait(cx, |_| !events.borrow().is_empty());
    assert!(matches!(
        rig.events.borrow().as_slice(),
        [MarkdownDraftEvent::Saved(_)]
    ));
    rig.finish(cx);
}

#[gpui::test]
fn ac3_possible_existing_songs_show_the_ordinary_count_and_preview(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-possible", &["Tinapay sa Langit"], TINAPAY);
    assert_eq!(rig.view.read_with(cx, |view, _| view.candidates().len()), 1);
    assert!(present(cx, "markdown-possible"));
    click(cx, "button-1 possible existing Song");
    assert!(present(cx, "markdown-candidate-0"));
    assert!(present(cx, "row-Tinapay sa Langit"));
    tap(cx, "markdown-candidate-0");
    assert_eq!(rig.depth(cx), 1);
    assert_eq!(
        cx.update(|_, cx| rig.overlays.top(cx)).as_deref(),
        Some("Tinapay sa Langit")
    );
    assert_eq!(
        cx.update(|_, cx| rig.overlays.top_description(cx))
            .as_deref(),
        Some("Existing Song \u{b7} preview only")
    );
    for part in [
        "new-song-preview",
        "button-Back",
        "button-Use this existing Song",
    ] {
        assert!(present(cx, part), "{part}");
    }
    // Back keeps the draft exactly as it was.
    tap(cx, "button-Back");
    assert_eq!(rig.depth(cx), 0);
    assert_eq!(
        rig.view.read_with(cx, |view, _| view.target()),
        SongChoice::New
    );
    assert_eq!(rig.text(cx), TINAPAY);
    assert!(present(cx, "button-Save song"));
    // Retitling the draft recomputes the evidence from the parsed Song.
    let title = TINAPAY.find("Tinapay sa Langit").expect("title");
    type_over(
        &rig,
        cx,
        title..title + "Tinapay sa Langit".len(),
        "Zzz Unknown",
    );
    parsed(&rig, cx);
    assert!(
        rig.view
            .read_with(cx, |view, _| view.candidates().is_empty())
    );
    assert!(!present(cx, "markdown-possible"));
    rig.finish(cx);
}

#[gpui::test]
fn ac3_use_this_existing_song_saves_one_arrangement_under_it(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-use", &["Tinapay sa Langit"], TINAPAY);
    let existing = rig.songs[0];
    click(cx, "button-1 possible existing Song");
    tap(cx, "markdown-candidate-0");
    tap(cx, "button-Use this existing Song");
    assert_eq!(rig.depth(cx), 0, "the preview closed");
    assert_eq!(
        rig.view.read_with(cx, |view, _| view.target()),
        SongChoice::Existing(existing)
    );
    assert!(present(cx, "button-Save to Tinapay sa Langit"));
    assert!(!present(cx, "button-Save song"));
    assert_eq!(rig.arrangements_of(cx, existing), 0);
    click(cx, "button-Save to Tinapay sa Langit");
    let events = rig.events.clone();
    wait(cx, |_| !events.borrow().is_empty());
    let saved = match rig.events.borrow().as_slice() {
        [MarkdownDraftEvent::Saved(saved)] => *saved,
        other => panic!("one Saved event, got {other:?}"),
    };
    let saved = &saved;
    assert_eq!(rig.library_songs(cx), 1, "no second Song was made");
    assert_eq!(rig.arrangements_of(cx, existing), 1);
    let owner = rig.store.read_with(cx, |store, _| {
        store.library().arrangement(*saved).map(|a| a.song())
    });
    assert_eq!(owner, Ok(existing));
    // The way back to a new Song stays one action away.
    rig.finish(cx);
}

#[gpui::test]
fn ac2_editing_the_repeat_label_resolves_it_and_updates_the_preview(cx: &mut TestAppContext) {
    let text =
        "---\ntitle: Gloria\nlanguage: English\n---\n[Verse]\nGlory to God\n[Repeat: Coda]\n";
    let (rig, cx) = open(cx, "wp10-repeat", &[], text);
    assert_eq!(
        sequence(&rig, cx),
        ["01 Verse", "02 [Repeat: Coda] \u{b7} not found"]
    );
    assert_eq!(rows(&rig, cx), ["Line 7 \u{b7} No section is called Coda."]);
    assert_eq!(
        preview::section_heading(Some("Verse"), "Glory to God"),
        "Verse \u{b7} 1 slide"
    );
    assert_eq!(
        preview::section_heading(None, "A\n\nB"),
        "Unlabeled \u{b7} 2 slides"
    );
    let at = text.find("Coda").expect("label");
    type_over(&rig, cx, at..at + 4, "Verse");
    parsed(&rig, cx);
    assert_eq!(sequence(&rig, cx), ["01 Verse", "02 Verse \u{b7} repeat"]);
    assert!(rows(&rig, cx).is_empty());
    assert!(present(cx, "markdown-sequence-1"));
    assert!(rig.view.read_with(cx, |view, cx| view.has_edits(cx)));
    rig.finish(cx);
}

#[gpui::test]
fn the_newest_parse_wins_and_keeps_one_identity(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-newest", &[], GLORIA);
    let at = GLORIA.find("Gloria").expect("title");
    // Two edits before the first parse is applied: only the last may show.
    type_over(&rig, cx, at..at + 6, "X");
    cx.simulate_input("Y");
    parsed(&rig, cx);
    assert_eq!(
        rig.view
            .read_with(cx, |view, _| view.parsed().document.song.title.clone()),
        "XY"
    );
    let ids = rig.view.read_with(cx, |view, _| {
        (
            view.parsed().document.id,
            view.parsed().document.arrangements[0].id,
        )
    });
    assert_eq!(ids, (song_id(0xa1), arrangement_id(0xa2)));
    rig.finish(cx);
}

#[gpui::test]
fn ac2_an_unresolved_repeat_still_saves_as_an_incomplete_arrangement(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-incomplete", &[], NOISY);
    assert!(rig.view.read_with(cx, |view, _| view.can_save()));
    click(cx, "button-Save song");
    let events = rig.events.clone();
    wait(cx, |_| !events.borrow().is_empty());
    let saved = match rig.events.borrow().as_slice() {
        [MarkdownDraftEvent::Saved(saved)] => *saved,
        other => panic!("one Saved event, got {other:?}"),
    };
    let saved = &saved;
    let (labels, coda, state) = rig.store.read_with(cx, |store, _| {
        let stored = store.library().arrangement(*saved).expect("stored");
        let labels: Vec<Option<String>> = stored
            .sections()
            .iter()
            .map(|section| section.label().map(str::to_owned))
            .collect();
        let coda = stored.sections()[1].lyrics().to_owned();
        let state = readiness(&stored.lyrics(), &BTreeSet::new()).state;
        (labels, coda, state)
    });
    assert_eq!(labels, [Some("Verse".to_owned()), Some("Coda".to_owned())]);
    assert_eq!(coda, "");
    assert_eq!(state, ReadinessState::Incomplete);
    rig.finish(cx);
}

#[gpui::test]
fn ac2_save_writes_the_exact_sections_sequence_and_slide_boundaries(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-exact", &[], REPEATS);
    assert_eq!(
        sequence(&rig, cx),
        [
            "01 Verse 1",
            "02 Chorus",
            "03 Chorus \u{b7} repeat",
            "04 Verse 1 \u{b7} repeat"
        ]
    );
    click(cx, "button-Save song");
    let events = rig.events.clone();
    wait(cx, |_| !events.borrow().is_empty());
    let saved = match rig.events.borrow().as_slice() {
        [MarkdownDraftEvent::Saved(saved)] => *saved,
        other => panic!("one Saved event, got {other:?}"),
    };
    let saved = &saved;
    let (sections, order, song) = rig.store.read_with(cx, |store, _| {
        let stored = store.library().arrangement(*saved).expect("stored");
        let sections: Vec<(Option<String>, String)> = stored
            .sections()
            .iter()
            .map(|s| (s.label().map(str::to_owned), s.lyrics().to_owned()))
            .collect();
        let order: Vec<Option<String>> = stored
            .sequence()
            .iter()
            .map(|occurrence| {
                stored
                    .sections()
                    .iter()
                    .find(|section| section.id() == occurrence.section())
                    .and_then(|section| section.label().map(str::to_owned))
            })
            .collect();
        let song = store
            .library()
            .song(stored.song())
            .map(|s| s.title().to_owned());
        (sections, order, song)
    });
    assert_eq!(
        sections,
        [
            (
                Some("Verse 1".to_owned()),
                "First line\n\nsecond slide of verse".to_owned()
            ),
            (Some("Chorus".to_owned()), "Sing it out".to_owned()),
        ]
    );
    let name = |label: &str| Some(label.to_owned());
    assert_eq!(
        order,
        [
            name("Verse 1"),
            name("Chorus"),
            name("Chorus"),
            name("Verse 1")
        ]
    );
    assert_eq!(song.as_deref(), Ok("Repeats"));
    rig.finish(cx);
}

#[gpui::test]
fn discard_with_edits_confirms_and_without_edits_closes(cx: &mut TestAppContext) {
    // Without edits it closes at once.
    let (rig, cx) = open(cx, "wp10-discard-clean", &[], GLORIA);
    click(cx, "button-Discard draft");
    assert_eq!(
        rig.events.borrow().as_slice(),
        [MarkdownDraftEvent::Discarded]
    );
    assert_eq!(rig.depth(cx), 0, "nothing was asked");
    rig.finish(cx);
}

#[gpui::test]
fn discard_with_edits_asks_and_keep_editing_keeps_the_draft(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-discard-edited", &[], GLORIA);
    let at = GLORIA.find("Glory").expect("lyrics");
    type_over(&rig, cx, at..at, "O ");
    parsed(&rig, cx);
    let edited = rig.text(cx);
    tap(cx, "button-Discard draft");
    assert!(rig.events.borrow().is_empty(), "asked first");
    assert_eq!(rig.depth(cx), 1);
    assert_eq!(
        cx.update(|_, cx| rig.overlays.top(cx)).as_deref(),
        Some("Discard this draft?")
    );
    assert_eq!(
        cx.update(|_, cx| rig.overlays.top_description(cx))
            .as_deref(),
        Some("Nothing from gloria.md has been saved.")
    );
    tap(cx, "button-Keep editing");
    assert_eq!(rig.depth(cx), 0);
    assert!(rig.events.borrow().is_empty());
    assert_eq!(rig.text(cx), edited, "the draft is untouched");
    // Asking again and confirming discards.
    tap(cx, "button-Discard draft");
    assert_eq!(rig.depth(cx), 1);
    tap(cx, "button-Discard");
    assert_eq!(
        rig.events.borrow().as_slice(),
        [MarkdownDraftEvent::Discarded]
    );
    assert_eq!(rig.library_songs(cx), 0);
    rig.finish(cx);
}

#[gpui::test]
fn nothing_is_written_before_save(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-nothing", &["Tinapay sa Langit"], TINAPAY);
    let existing = rig.songs[0];
    let at = TINAPAY.find("Bread").expect("lyrics");
    type_over(&rig, cx, at..at, "Our ");
    parsed(&rig, cx);
    click(cx, "button-1 possible existing Song");
    tap(cx, "markdown-candidate-0");
    tap(cx, "button-Use this existing Song");
    assert_eq!(rig.library_songs(cx), 1);
    assert_eq!(rig.arrangements_of(cx, existing), 0);
    assert!(rig.events.borrow().is_empty());
    // Not even after the draft is discarded.
    tap(cx, "button-Discard draft");
    tap(cx, "button-Discard");
    assert_eq!(rig.library_songs(cx), 1);
    assert_eq!(rig.arrangements_of(cx, existing), 0);
    rig.finish(cx);
}

#[gpui::test]
fn a_refused_save_stays_in_the_header_and_keeps_the_draft(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-refused", &["Tinapay sa Langit"], TINAPAY);
    let existing = rig.songs[0];
    click(cx, "button-1 possible existing Song");
    tap(cx, "markdown-candidate-0");
    tap(cx, "button-Use this existing Song");
    // The Song is archived behind the draft's back.
    let task = rig
        .store
        .update(cx, |store, cx| store.archive_song(existing, cx));
    cx.foreground_executor().block_test(task).expect("archived");
    cx.run_until_parked();
    click(cx, "button-Save to Tinapay sa Langit");
    let view = rig.view.clone();
    wait(cx, |cx| {
        view.read_with(cx, |view, _| view.refusal().is_some() && !view.busy())
    });
    assert!(present(cx, "markdown-refusal"));
    assert!(rig.events.borrow().is_empty());
    assert_eq!(rig.arrangements_of(cx, existing), 0);
    assert_eq!(rig.text(cx), TINAPAY, "the draft stays");
    // Choosing a new Song instead clears the refusal and can save.
    click(cx, "button-Add as a new Song");
    assert!(rig.view.read_with(cx, |view, _| view.refusal().is_none()));
    click(cx, "button-Save song");
    let events = rig.events.clone();
    wait(cx, |_| !events.borrow().is_empty());
    assert_eq!(rig.library_songs(cx), 2);
    rig.finish(cx);
}

#[gpui::test]
fn the_draft_is_keyboard_reachable_by_f6_regions(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp10-f6", &[], GLORIA);
    let focused = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| rig.view.read(cx).focused_region(window, cx))
            .map(|name| name.to_string())
    };
    let mut order = vec![focused(cx)];
    for _ in 0..4 {
        cx.simulate_keystrokes("f6");
        cx.run_until_parked();
        order.push(focused(cx));
    }
    let order: Vec<_> = order.into_iter().flatten().collect();
    assert_eq!(
        order,
        [
            "Main editor",
            "Inspector",
            "Workspace actions",
            "Primary list",
            "Main editor"
        ]
    );
    rig.finish(cx);
}
