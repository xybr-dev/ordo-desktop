//! Review Song import behaviour tests, over a real parish store in a
//! temporary directory, a real window, and real clicks and keys.

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use domain::{
    identity::{ArrangementId, EntityId, SongId},
    song_library::import::{
        Choices, Classification, ImportArrangement, ImportDocument, ImportSection, Outcome,
        SequenceEntry, SongChoice, SongIdentity,
    },
};
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement as _, Render,
    Styled as _, Task, TestAppContext, VisualTestContext, Window,
};
use storage::exchange::{ManifestOverrides, encode_song_import, pack_for_tests};
use ui::{
    environment::{ResolvedAppearance, UiEnvironment, UiPreferences},
    overlay::OverlayHost,
};

use super::{FilePicker, ImportReview, ImportReviewEvent, evidence, list};
use crate::songs::{
    LibraryStore, StorageSlot,
    choirs::tests::{click, open_parish, temp_dir, wait},
};

const COULDNT: &str = "Couldn't import. Nothing was changed.";

fn song_id(byte: u8) -> SongId {
    SongId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

fn arrangement_id(byte: u8) -> ArrangementId {
    ArrangementId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

/// A one-Arrangement document: a Verse and a Chorus, sung Verse, Chorus.
fn doc(song: u8, arrangement: u8, title: &str, verse: &str) -> ImportDocument {
    ImportDocument {
        id: song_id(song),
        song: domain::song_library::SongDraft::titled(title),
        arrangements: vec![ImportArrangement {
            id: arrangement_id(arrangement),
            title: Some("Imported".to_owned()),
            languages: vec!["English".to_owned()],
            choirs: Vec::new(),
            rendition_attribution: None,
            occasions: Vec::new(),
            notes: None,
            sections: vec![
                ImportSection {
                    label: Some("Verse".to_owned()),
                    lyrics: verse.to_owned(),
                },
                ImportSection {
                    label: Some("Chorus".to_owned()),
                    lyrics: "Sing it again".to_owned(),
                },
            ],
            sequence: vec![SequenceEntry::Section(0), SequenceEntry::Section(1)],
            warnings: Vec::new(),
        }],
        warnings: Vec::new(),
    }
}

fn ordo_bytes(document: &ImportDocument) -> Vec<u8> {
    let payload = encode_song_import(document).to_string().into_bytes();
    pack_for_tests(&ManifestOverrides::default(), &payload, &[])
}

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write the file");
    path
}

/// Answers every pick with the next queued answer.
#[derive(Default)]
struct FakePicker {
    answers: RefCell<Vec<Option<Vec<PathBuf>>>>,
}

impl FilePicker for FakePicker {
    fn pick(&self, _multiple: bool, _cx: &mut App) -> Task<Option<Vec<PathBuf>>> {
        let answer = {
            let mut answers = self.answers.borrow_mut();
            if answers.is_empty() {
                None
            } else {
                answers.remove(0)
            }
        };
        Task::ready(answer)
    }
}

/// Hosts the review like the shell does: draws its frame, and takes focus
/// back to the invoker when it is cancelled.
struct Host {
    review: Entity<ImportReview>,
    invoker: FocusHandle,
    overlays: OverlayHost,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
        let frame = self.review.update(cx, |review, cx| {
            review.render_frame(environment, window, cx)
        });
        gpui::div()
            .size_full()
            .child(frame)
            .child(self.overlays.layer(environment, window, cx))
    }
}

struct Rig {
    review: Entity<ImportReview>,
    store: Entity<LibraryStore>,
    slot: StorageSlot,
    dir: PathBuf,
    files: PathBuf,
    events: Rc<RefCell<Vec<ImportReviewEvent>>>,
    picker: Rc<FakePicker>,
    invoker: FocusHandle,
    overlays: OverlayHost,
}

impl Rig {
    fn library_songs(&self, cx: &mut VisualTestContext) -> usize {
        self.store
            .read_with(cx, |store, _| store.library().songs().count())
    }

    fn song_arrangements(&self, cx: &mut VisualTestContext, song: SongId) -> usize {
        self.store.read_with(cx, |store, _| {
            store
                .library()
                .arrangements_for_song(song)
                .map_or(0, |list| list.len())
        })
    }

    fn finish(&self, cx: &mut VisualTestContext) {
        let outcome = cx.foreground_executor().block_test(self.slot.shutdown());
        assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.files);
    }
}

fn stage(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    paths: Vec<PathBuf>,
) -> crate::songs::store::ImportStaging {
    let task = store.update(cx, |store, cx| store.stage_import(paths, cx));
    cx.foreground_executor().block_test(task)
}

/// Opens a review of `batch` (name and bytes) over a parish that already holds
/// the `seed` documents.
fn open<'a>(
    cx: &'a mut TestAppContext,
    tag: &str,
    seed: &[ImportDocument],
    batch: &[(&str, Vec<u8>)],
) -> (Rig, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    let dir = temp_dir(tag);
    let files = temp_dir(&format!("{tag}-files"));
    let (slot, store) = open_parish(cx, &dir);
    for (index, document) in seed.iter().enumerate() {
        let path = write(&files, &format!("seed-{index}.ordo"), &ordo_bytes(document));
        let staging = stage(cx, &store, vec![path]);
        let task = store.update(cx, |store, cx| {
            store.commit_import(&staging, Choices::default(), cx)
        });
        cx.foreground_executor().block_test(task).expect("seeded");
    }
    let paths = batch
        .iter()
        .map(|(name, bytes)| write(&files, name, bytes))
        .collect();
    let staging = stage(cx, &store, paths);
    cx.update(crate::init_ui).expect("ui");
    let picker = Rc::new(FakePicker::default());
    let events = Rc::new(RefCell::new(Vec::new()));
    let for_window = (store.clone(), picker.clone(), events.clone());
    let (host, cx) = cx.add_window_view(move |window, cx| {
        let (store, picker, events) = for_window;
        let overlays = OverlayHost::new(cx);
        let review = cx.new(|cx| ImportReview::new(staging, store, picker, window, cx));
        review.update(cx, |review, _| review.set_overlays(overlays.clone()));
        cx.observe(&review, |_, _, cx| cx.notify()).detach();
        let invoker = cx.focus_handle();
        let log = events.clone();
        cx.subscribe_in(
            &review,
            window,
            move |host: &mut Host, _, event, window, cx| {
                log.borrow_mut().push(event.clone());
                if matches!(event, ImportReviewEvent::Cancelled) {
                    window.focus(&host.invoker, cx);
                }
            },
        )
        .detach();
        Host {
            review,
            invoker,
            overlays,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    cx.run_until_parked();
    let (review, invoker, overlays) = host.read_with(cx, |host, _| {
        (
            host.review.clone(),
            host.invoker.clone(),
            host.overlays.clone(),
        )
    });
    (
        Rig {
            review,
            store,
            slot,
            dir,
            files,
            events,
            picker,
            invoker,
            overlays,
        },
        cx,
    )
}

fn present(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

fn chips(rig: &Rig, cx: &mut VisualTestContext) -> Vec<&'static str> {
    rig.review.read_with(cx, |review, _| {
        (0..review.documents().len())
            .map(|index| list::chip(review, index))
            .collect()
    })
}

fn tinapay_seed() -> Vec<ImportDocument> {
    vec![doc(1, 0x11, "Tinapay sa Langit", "Bread from heaven")]
}

/// A batch whose one document resembles the seeded Song but has its own
/// identity, so the operator must say which Song it is.
fn near_song_batch() -> Vec<(&'static str, Vec<u8>)> {
    vec![(
        "near.ordo",
        ordo_bytes(&doc(2, 0x12, "Tinapay, sa Langit", "Bread from heaven")),
    )]
}

/// The same Song and Arrangement with reworded lyrics: an Update candidate.
fn reworded_batch() -> Vec<(&'static str, Vec<u8>)> {
    vec![(
        "reworded.ordo",
        ordo_bytes(&doc(1, 0x11, "Tinapay sa Langit", "Bread, reworded")),
    )]
}

#[gpui::test]
fn ac3_ordo_files_open_one_batch_review_with_every_document_listed(cx: &mut TestAppContext) {
    let batch = vec![
        ("a.ordo", ordo_bytes(&doc(1, 0x11, "Alleluia", "Sing"))),
        ("b.ordo", ordo_bytes(&doc(2, 0x12, "Benedictus", "Bless"))),
        ("c.ordo", ordo_bytes(&doc(3, 0x13, "Credo", "Believe"))),
    ];
    let (rig, cx) = open(cx, "wp8-ac3", &[], &batch);
    assert_eq!(rig.review.read_with(cx, |r, _| r.documents().len()), 3);
    for selector in [
        "import-doc-0",
        "import-doc-1",
        "import-doc-2",
        "import-count",
    ] {
        assert!(present(cx, selector), "{selector}");
    }
    assert_eq!(chips(&rig, cx), ["Add new Song"; 3]);
    assert!(present(cx, "button-Import approved"));
    rig.finish(cx);
}

#[gpui::test]
fn ac4_each_row_shows_its_classification_chip_and_the_detail_its_evidence(cx: &mut TestAppContext) {
    let mut attach = doc(1, 0x21, "Tinapay sa Langit", "Completely other words");
    attach.arrangements[0].title = Some("Choral".to_owned());
    attach.arrangements[0].languages = vec!["Tagalog".to_owned()];
    attach.arrangements[0].sections.truncate(1);
    attach.arrangements[0].sequence = vec![SequenceEntry::Section(0)];
    let batch = vec![
        ("a-new.ordo", ordo_bytes(&doc(7, 0x17, "Alleluia", "Sing"))),
        ("b-same.ordo", ordo_bytes(&tinapay_seed()[0])),
        ("c-attach.ordo", ordo_bytes(&attach)),
    ];
    let (rig, cx) = open(cx, "wp8-ac4", &tinapay_seed(), &batch);
    let seen = chips(&rig, cx);
    assert_eq!(seen[0], "Add new Song");
    assert_eq!(seen[1], "Skip unchanged");
    assert!(
        matches!(seen[2], "Attach Arrangement" | "Needs decision"),
        "{seen:?}"
    );
    click(cx, "import-doc-1");
    assert_eq!(rig.review.read_with(cx, |r, _| r.selected_document()), 1);
    for selector in [
        "import-card-imported",
        "import-card-library",
        "import-song-evidence",
    ] {
        assert!(present(cx, selector), "{selector}");
    }
    rig.review.read_with(cx, |review, cx| {
        let document = review.review_of(1).expect("valid");
        assert!(matches!(document.identity, SongIdentity::Existing { .. }));
        let library = review.store.read(cx).library();
        let (kind, sentence) = evidence::song_sentence(
            &document.identity,
            match &review.documents()[1].content {
                domain::song_library::import::Staged::Valid(valid) => valid,
                _ => unreachable!("valid"),
            },
            library,
        );
        assert!(!kind.is_empty() && !sentence.is_empty());
    });
    rig.finish(cx);
}

#[gpui::test]
fn ac4_a_staged_action_can_be_changed_with_the_select(cx: &mut TestAppContext) {
    let batch = vec![("same.ordo", ordo_bytes(&tinapay_seed()[0]))];
    let (rig, cx) = open(cx, "wp8-ac4-select", &tinapay_seed(), &batch);
    assert_eq!(chips(&rig, cx), ["Skip unchanged"]);
    click(cx, "button-Staged action: Skip unchanged");
    assert!(present(cx, "choice-1"), "the Select lists its actions");
    click(cx, "choice-1");
    let chosen = rig
        .review
        .read_with(cx, |r, _| r.choices().arrangements.get(&(0, 0)).copied());
    assert_eq!(chosen, Some(Outcome::AttachAsAnother));
    assert_eq!(chips(&rig, cx), ["Attach Arrangement"]);
    assert!(rig.review.read_with(cx, |r, _| r.can_commit()));
    rig.finish(cx);
}

#[gpui::test]
fn ac5_a_song_identity_decision_is_made_once_for_the_document(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp8-ac5-song", &tinapay_seed(), &near_song_batch());
    assert_eq!(chips(&rig, cx), ["Needs decision"]);
    assert!(present(cx, "import-identity"));
    assert!(!rig.review.read_with(cx, |r, _| r.can_commit()));
    click(cx, "radio-import-song-choice-0-0");
    wait(cx, |cx| {
        rig.review.read_with(cx, |r, _| {
            !r.busy()
                && r.choices().songs.get(&0) == Some(&SongChoice::Existing(song_id(1)))
                && r.review_of(0)
                    .is_some_and(|d| !d.arrangements[0].offered.is_empty())
        })
    });
    // One choice for the document, none per Arrangement.
    assert_eq!(rig.review.read_with(cx, |r, _| r.choices().songs.len()), 1);
    rig.finish(cx);
}

#[gpui::test]
fn ac5_arrangement_outcomes_wait_for_the_song_choice(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp8-ac5-wait", &tinapay_seed(), &near_song_batch());
    rig.review.read_with(cx, |review, _| {
        let document = review.review_of(0).expect("valid");
        assert!(matches!(
            document.identity,
            SongIdentity::NeedsDecision { .. }
        ));
        assert_eq!(
            document.arrangements[0].classification,
            Classification::NeedsDecision
        );
        assert!(
            document.arrangements[0].offered.is_empty(),
            "nothing is offered before the Song is chosen"
        );
    });
    // No Outcome is offered, and Compare is not available, until the Song is chosen.
    assert!(!present(cx, "radio-import-outcome-0-0"));
    assert!(!present(cx, "radio-import-outcome-0-1"));
    if present(cx, "button-Compare contents") {
        click(cx, "button-Compare contents");
    }
    assert!(
        rig.review.read_with(cx, |review, _| review.compare().is_none()),
        "Compare does nothing before the Song is chosen"
    );
    // Add as a new Song: the option after the one candidate.
    click(cx, "radio-import-song-choice-0-1");
    wait(cx, |cx| {
        rig.review.read_with(cx, |r, _| {
            !r.busy()
                && r.choices().songs.get(&0) == Some(&SongChoice::New)
                && r.review_of(0).is_some_and(|d| {
                    d.arrangements[0].classification != Classification::NeedsDecision
                })
        })
    });
    assert_eq!(chips(&rig, cx), ["Add new Song"]);
    assert!(rig.review.read_with(cx, |r, _| r.can_commit()));
    rig.finish(cx);
}

#[gpui::test]
fn ac7_a_near_duplicate_offers_add_as_new_update_existing_or_skip_with_recommended(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = open(cx, "wp8-ac7", &tinapay_seed(), &reworded_batch());
    rig.review.read_with(cx, |review, _| {
        let arrangement = &review.review_of(0).expect("valid").arrangements[0];
        assert_eq!(arrangement.classification, Classification::NeedsDecision);
        assert_eq!(arrangement.recommended, Some(Outcome::UpdateExisting));
        let mut offered = arrangement.offered.clone();
        offered.sort_by_key(|outcome| format!("{outcome:?}"));
        assert_eq!(
            offered,
            [Outcome::AddAsNew, Outcome::Skip, Outcome::UpdateExisting]
        );
    });
    for selector in [
        "radio-import-outcome-0-0",
        "radio-import-outcome-0-1",
        "radio-import-outcome-0-2",
    ] {
        assert!(present(cx, selector), "{selector}");
    }
    assert!(!present(cx, "radio-import-outcome-0-3"));
    assert_eq!(chips(&rig, cx), ["Needs decision"]);
    rig.finish(cx);
}

#[gpui::test]
fn ac7_update_existing_discloses_the_choirs_inline(cx: &mut TestAppContext) {
    let mut changed = doc(1, 0x11, "Tinapay sa Langit", "Bread, reworded");
    changed.arrangements[0].choirs = vec!["Koro Sto. Nino".to_owned()];
    let batch = vec![("choirs.ordo", ordo_bytes(&changed))];
    let (rig, cx) = open(cx, "wp8-ac7-choirs", &tinapay_seed(), &batch);
    assert!(present(cx, "import-card-imported"));
    rig.review.update(cx, |review, cx| {
        let offered = review.review_of(0).expect("valid").arrangements[0]
            .offered
            .clone();
        let at = offered
            .iter()
            .position(|outcome| *outcome == Outcome::UpdateExisting)
            .expect("Update is offered");
        review.choose_outcome(0, 0, at, cx);
    });
    cx.run_until_parked();
    let summary = rig
        .review
        .read_with(cx, |review, _| evidence::summary(&review.tally()));
    assert!(summary.contains("1 update"), "{summary}");
    assert!(summary.contains("1 new Choir"), "{summary}");
    assert_eq!(chips(&rig, cx), ["Update existing"]);
    rig.finish(cx);
}

#[gpui::test]
fn ac8_import_approved_is_disabled_until_every_decision_is_made(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp8-ac8-gate", &tinapay_seed(), &reworded_batch());
    let before = rig.song_arrangements(cx, song_id(1));
    click(cx, "button-Import approved");
    assert!(
        rig.events.borrow().is_empty(),
        "a blocked button does nothing"
    );
    assert_eq!(rig.song_arrangements(cx, song_id(1)), before);
    // Decide: the first offered option.
    click(cx, "radio-import-outcome-0-0");
    assert!(rig.review.read_with(cx, |r, _| r.can_commit()));
    click(cx, "button-Import approved");
    wait(cx, |_| !rig.events.borrow().is_empty());
    assert!(matches!(
        rig.events.borrow()[0],
        ImportReviewEvent::Committed(_)
    ));
    rig.finish(cx);
}

#[gpui::test]
fn ac8_an_invalid_document_is_listed_with_its_reason_and_technical_details(
    cx: &mut TestAppContext,
) {
    let batch = vec![
        ("a-broken.ordo", b"this is not an archive".to_vec()),
        ("b-good.ordo", ordo_bytes(&doc(7, 0x17, "Alleluia", "Sing"))),
    ];
    let (rig, cx) = open(cx, "wp8-ac8-invalid", &[], &batch);
    assert_eq!(chips(&rig, cx), ["Invalid", "Add new Song"]);
    click(cx, "import-doc-0");
    assert!(present(cx, "import-invalid"));
    assert!(present(cx, "button-Remove from review"));
    assert!(present(cx, "button-Choose another file"));
    click(cx, "button-View technical details");
    assert!(present(cx, "import-technical"));
    rig.review.read_with(cx, |review, _| {
        let domain::song_library::import::Staged::Invalid(reasons) = &review.documents()[0].content
        else {
            panic!("invalid");
        };
        assert!(!evidence::technical_line(&reasons[0]).is_empty());
        assert!(
            !evidence::invalid_sentence(reasons, "a-broken.ordo").is_empty(),
            "the reason is put in words"
        );
    });
    // The good document still imports.
    assert!(rig.review.read_with(cx, |r, _| r.can_commit()));
    rig.finish(cx);
}

#[gpui::test]
fn ac8_remove_from_review_and_choose_another_file_change_only_staging(cx: &mut TestAppContext) {
    let batch = vec![
        ("a-broken.ordo", b"nope".to_vec()),
        ("b-broken.ordo", b"still nope".to_vec()),
        ("c-good.ordo", ordo_bytes(&doc(7, 0x17, "Alleluia", "Sing"))),
    ];
    let (rig, cx) = open(cx, "wp8-ac8-fix", &[], &batch);
    let songs = rig.library_songs(cx);
    click(cx, "import-doc-0");
    click(cx, "button-Remove from review");
    wait(cx, |cx| {
        rig.review
            .read_with(cx, |r, _| r.documents().len() == 2 && !r.busy())
    });
    assert_eq!(rig.library_songs(cx), songs);

    let replacement = write(
        &rig.files,
        "d-fixed.ordo",
        &ordo_bytes(&doc(8, 0x18, "Benedictus", "Bless")),
    );
    rig.picker
        .answers
        .borrow_mut()
        .push(Some(vec![replacement]));
    click(cx, "import-doc-0");
    click(cx, "button-Choose another file");
    wait(cx, |cx| {
        rig.review.read_with(cx, |r, _| {
            r.documents().iter().any(|d| d.source.contains("d-fixed"))
        })
    });
    assert_eq!(rig.library_songs(cx), songs, "nothing was written");
    assert!(rig.events.borrow().is_empty());
    rig.finish(cx);
}

#[gpui::test]
fn ac8_cancel_import_changes_nothing_and_returns_focus(cx: &mut TestAppContext) {
    let batch = vec![("a.ordo", ordo_bytes(&doc(7, 0x17, "Alleluia", "Sing")))];
    let (rig, cx) = open(cx, "wp8-ac8-cancel", &[], &batch);
    click(cx, "button-Cancel import");
    assert_eq!(*rig.events.borrow(), [ImportReviewEvent::Cancelled]);
    assert_eq!(rig.library_songs(cx), 0);
    let focused = cx.update(|window, _| rig.invoker.is_focused(window));
    assert!(focused, "focus returns to the invoker");
    rig.finish(cx);
}

#[gpui::test]
fn ac9_a_failed_commit_keeps_the_review_open_with_the_refusal(cx: &mut TestAppContext) {
    let batch = vec![("a.ordo", ordo_bytes(&doc(7, 0x17, "Alleluia", "Sing")))];
    let (rig, cx) = open(cx, "wp8-ac9", &[], &batch);
    let future = rig
        .slot
        .with(|storage| storage.inject_write_failure(1))
        .expect("storage is open");
    cx.foreground_executor().block_test(future).expect("armed");
    click(cx, "button-Import approved");
    wait(cx, |cx| {
        rig.review.read_with(cx, |r, _| r.refusal().is_some())
    });
    assert_eq!(
        rig.review
            .read_with(cx, |r, _| r.refusal().map(ToString::to_string)),
        Some(COULDNT.to_owned())
    );
    assert!(present(cx, "import-refusal"));
    assert!(rig.events.borrow().is_empty(), "the review stays open");
    assert!(!rig.review.read_with(cx, |r, _| r.busy()));
    assert_eq!(rig.library_songs(cx), 0);
    rig.finish(cx);
}

#[gpui::test]
fn ac10_a_batch_of_unchanged_documents_has_nothing_to_import(cx: &mut TestAppContext) {
    let batch = vec![("same.ordo", ordo_bytes(&tinapay_seed()[0]))];
    let (rig, cx) = open(cx, "wp8-ac10", &tinapay_seed(), &batch);
    assert_eq!(chips(&rig, cx), ["Skip unchanged"]);
    let summary = rig
        .review
        .read_with(cx, |review, _| evidence::summary(&review.tally()));
    assert_eq!(summary, "Everything is already in the Library.");
    assert!(!rig.review.read_with(cx, |r, _| r.can_commit()));
    click(cx, "button-Import approved");
    assert!(rig.events.borrow().is_empty());
    rig.finish(cx);
}

#[gpui::test]
fn the_review_is_keyboard_reachable_by_f6_regions(cx: &mut TestAppContext) {
    let batch = vec![("a.ordo", ordo_bytes(&doc(7, 0x17, "Alleluia", "Sing")))];
    let (rig, cx) = open(cx, "wp8-f6", &[], &batch);
    let focused = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| rig.review.read(cx).focused_region(window, cx))
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
            "Primary list",
            "Main editor",
            "Review summary",
            "Workspace actions",
            "Primary list"
        ]
    );
    rig.finish(cx);
}

// ---- WP9: Compare contents ----

/// A second Arrangement of the seeded Song, with its own title, language and
/// words, so the Song has two local Arrangements and nothing resembles it.
fn second_arrangement() -> ImportDocument {
    let mut second = doc(1, 0x13, "Tinapay sa Langit", "Another verse entirely");
    second.arrangements[0].title = Some("Second".to_owned());
    second.arrangements[0].languages = vec!["Cebuano".to_owned()];
    second
}

fn two_arrangement_seed() -> Vec<ImportDocument> {
    vec![
        doc(1, 0x11, "Tinapay sa Langit", "Bread from heaven"),
        second_arrangement(),
    ]
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

fn depth(rig: &Rig, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| rig.overlays.depth(cx))
}

/// Opens Compare contents from the document's detail with a real click.
fn open_comparison(rig: &Rig, cx: &mut VisualTestContext, button: &'static str) {
    tap(cx, button);
    assert_eq!(depth(rig, cx), 1, "one slideover is open");
}

fn comparison(rig: &Rig, cx: &mut VisualTestContext) -> Entity<super::ImportCompare> {
    rig.review
        .read_with(cx, |review, _| review.compare().cloned())
        .expect("the comparison is open")
}

#[gpui::test]
fn ac6_compare_contents_opens_one_read_only_wide_slideover(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp9-open", &tinapay_seed(), &reworded_batch());
    open_comparison(&rig, cx, "button-Compare contents");
    assert!(present(cx, "overlay-body-Compare contents"));
    assert!(present(cx, "compare-body"));
    let description = cx.update(|_, cx| rig.overlays.top_description(cx));
    assert_eq!(
        description.as_deref(),
        Some("Read-only \u{b7} the staged action doesn\u{2019}t change here")
    );
    // The slideover is wide: wider than a default one, which is 480 px.
    let width = cx
        .debug_bounds("overlay-body-Compare contents")
        .expect("body")
        .size
        .width;
    assert!(f32::from(width) > 600.0, "wide: {width:?}");
    // Asking again does not stack a second comparison.
    rig.review.update_in(cx, |review, window, cx| {
        review.open_compare(0, 0, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(depth(&rig, cx), 1);
    let titles =
        comparison(&rig, cx).read_with(cx, |compare, cx| compare.view().read(cx).titles().clone());
    assert_eq!(titles.left, "Imported \u{b7} reworded.ordo");
    assert_eq!(titles.right, "Library \u{b7} Imported");
    rig.finish(cx);
}

#[gpui::test]
fn ac6_it_has_overview_lyrics_sequence_and_slides(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp9-tabs", &tinapay_seed(), &reworded_batch());
    open_comparison(&rig, cx, "button-Compare contents");
    let view = comparison(&rig, cx).read_with(cx, |compare, _| compare.view().clone());
    let tab = |cx: &mut VisualTestContext| view.read_with(cx, |view, cx| view.tab(cx));
    assert_eq!(tab(cx), "Lyrics");
    for (selector, name) in [
        ("tab-0", "Overview"),
        ("tab-1", "Lyrics"),
        ("tab-2", "Sequence"),
        ("tab-3", "Slides"),
    ] {
        tap(cx, selector);
        assert_eq!(tab(cx), name);
    }
    tap(cx, "tab-2");
    assert!(present(cx, "compare-current-seq-0"));
    assert!(present(cx, "compare-revision-seq-1"));
    rig.finish(cx);
}

#[gpui::test]
fn ac6_changed_lines_and_occurrences_are_marked_against_the_nearest_candidate(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = open(cx, "wp9-marks", &tinapay_seed(), &reworded_batch());
    open_comparison(&rig, cx, "button-Compare contents");
    tap(cx, "tab-1");
    // The Verse is reworded; its section ids were aligned, so only its line
    // is marked and the Chorus is not.
    assert!(present(cx, "compare-current-line-0-0-changed"));
    assert!(present(cx, "compare-revision-line-0-0-changed"));
    assert!(present(cx, "compare-current-line-1-0"));
    assert!(!present(cx, "compare-current-line-1-0-changed"));
    tap(cx, "tab-2");
    assert!(
        !present(cx, "compare-current-seq-0-changed")
            && !present(cx, "compare-current-seq-1-changed"),
        "the Sequence is the same"
    );
    rig.finish(cx);
}

#[gpui::test]
fn ac6_nearest_match_switches_the_library_column(cx: &mut TestAppContext) {
    let (rig, cx) = open(
        cx,
        "wp9-nearest",
        &two_arrangement_seed(),
        &reworded_batch(),
    );
    open_comparison(&rig, cx, "button-Compare contents");
    let compare = comparison(&rig, cx);
    assert_eq!(
        compare.read_with(cx, |compare, _| compare.candidates()),
        ["Imported", "Second"]
    );
    let view = compare.read_with(cx, |compare, _| compare.view().clone());
    let right = |cx: &mut VisualTestContext| view.read_with(cx, |v, _| v.titles().right.clone());
    assert_eq!(right(cx), "Library \u{b7} Imported");
    let before = rig.review.read_with(cx, |review, _| {
        (review.selected_document(), review.choices().clone())
    });
    tap(cx, "button-Nearest match: Imported");
    tap(cx, "choice-1");
    assert_eq!(right(cx), "Library \u{b7} Second");
    assert_eq!(
        compare.read_with(cx, |compare, cx| compare.nearest(cx)),
        Some(arrangement_id(0x13))
    );
    let after = rig.review.read_with(cx, |review, _| {
        (review.selected_document(), review.choices().clone())
    });
    assert_eq!(before, after, "only the right column changed");
    rig.finish(cx);
}

#[gpui::test]
fn ac6_no_field_or_edit_control_exists_in_the_comparison(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp9-readonly", &tinapay_seed(), &reworded_batch());
    open_comparison(&rig, cx, "button-Compare contents");
    assert!(!present(cx, "text-area-control"));
    let outcomes = rig
        .review
        .read_with(cx, |review, _| review.choices().arrangements.clone());
    // Clicking every line of the comparison changes nothing.
    for selector in [
        "compare-current-line-0-0-changed",
        "compare-revision-line-0-0-changed",
        "compare-current-line-1-0",
    ] {
        tap(cx, selector);
    }
    assert_eq!(
        rig.review
            .read_with(cx, |review, _| review.choices().arrangements.clone()),
        outcomes
    );
    assert_eq!(depth(&rig, cx), 1);
    rig.finish(cx);
}

#[gpui::test]
fn ac6_closing_keeps_the_staged_action_selection_and_scroll(cx: &mut TestAppContext) {
    let batch = vec![
        ("a.ordo", ordo_bytes(&doc(7, 0x17, "Alleluia", "Sing"))),
        ("same.ordo", ordo_bytes(&tinapay_seed()[0])),
    ];
    let (rig, cx) = open(cx, "wp9-close", &tinapay_seed(), &batch);
    rig.review.update(cx, |review, cx| review.select(1, cx));
    cx.run_until_parked();
    tap(cx, "button-Staged action: Skip unchanged");
    tap(cx, "choice-1");
    let state = |rig: &Rig, cx: &mut VisualTestContext| {
        rig.review.read_with(cx, |review, _| {
            (
                review.selected_document(),
                review.choices().clone(),
                review.can_commit(),
            )
        })
    };
    let scroll = |rig: &Rig, cx: &mut VisualTestContext| {
        rig.review
            .read_with(cx, |review, _| review.detail_scroll.handle().offset())
    };
    let (before, offset) = (state(&rig, cx), scroll(&rig, cx));
    open_comparison(&rig, cx, "button-Compare contents");
    tap(cx, "button-Back to review");
    settle(cx);
    assert_eq!(depth(&rig, cx), 0, "the comparison closed");
    assert!(
        rig.review
            .read_with(cx, |review, _| review.compare().is_none())
    );
    assert_eq!(state(&rig, cx), before);
    assert_eq!(scroll(&rig, cx), offset);
    assert_eq!(chips(&rig, cx)[1], "Attach Arrangement");
    rig.finish(cx);
}

#[gpui::test]
fn ac6_slides_compose_off_the_gpui_thread(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx, "wp9-slides", &tinapay_seed(), &reworded_batch());
    open_comparison(&rig, cx, "button-Compare contents");
    tap(cx, "tab-3");
    wait(cx, |cx| {
        cx.update(|window, _| window.refresh());
        present(cx, "compare-current-slide-0") && present(cx, "compare-revision-slide-0")
    });
    let frames =
        comparison(&rig, cx).read_with(cx, |compare, cx| compare.view().read(cx).frames().clone());
    assert!(frames.read_with(cx, |frames, _| frames.held()) > 0);
    assert_eq!(frames.read_with(cx, |frames, _| frames.pending()), 0);
    rig.finish(cx);
}

#[gpui::test]
fn ac6_a_new_song_document_previews_with_no_match(cx: &mut TestAppContext) {
    let batch = vec![("new.ordo", ordo_bytes(&doc(7, 0x17, "Alleluia", "Sing")))];
    let (rig, cx) = open(cx, "wp9-new", &[], &batch);
    assert!(!present(cx, "button-Compare contents"));
    open_comparison(&rig, cx, "button-Preview document");
    let view = comparison(&rig, cx).read_with(cx, |compare, _| compare.view().clone());
    let (titles, none) = view.read_with(cx, |view, _| (view.titles().clone(), view.no_match()));
    assert_eq!(titles.left, "Imported \u{b7} new.ordo");
    assert_eq!(titles.right, "Library");
    assert!(none, "the right column says No match.");
    assert!(
        !comparison(&rig, cx).read_with(cx, |compare, _| compare.candidates().len() > 1),
        "no Nearest match select without candidates"
    );
    assert!(!present(cx, "button-Nearest match: Imported"));
    // Nothing on the left is marked against nothing.
    tap(cx, "tab-1");
    assert!(present(cx, "compare-current-line-0-0"));
    assert!(!present(cx, "compare-current-line-0-0-changed"));
    rig.finish(cx);
}

// ---- WP11: Import entry, file picking and shell frames ----

/// Answers each pick with the next scripted answer and counts the asks.
#[derive(Default)]
struct ScriptedPicker {
    answers: RefCell<Vec<Option<Vec<PathBuf>>>>,
    asked: std::cell::Cell<usize>,
    multiple: std::cell::Cell<Option<bool>>,
}

impl FilePicker for ScriptedPicker {
    fn pick(&self, multiple: bool, _cx: &mut App) -> Task<Option<Vec<PathBuf>>> {
        self.asked.set(self.asked.get() + 1);
        self.multiple.set(Some(multiple));
        let mut answers = self.answers.borrow_mut();
        Task::ready(if answers.is_empty() {
            None
        } else {
            answers.remove(0)
        })
    }
}

/// A shell on Library → Songs over a real parish, with a scripted picker.
struct ShellRig {
    shell: Entity<crate::shell::ShellView>,
    store: Entity<LibraryStore>,
    slot: StorageSlot,
    dir: PathBuf,
    files: PathBuf,
    picker: Rc<ScriptedPicker>,
}

impl ShellRig {
    fn answer(&self, paths: Vec<PathBuf>) {
        self.picker.answers.borrow_mut().push(Some(paths));
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        write(&self.files, name, bytes)
    }

    fn songs(&self, cx: &mut VisualTestContext) -> usize {
        self.store
            .read_with(cx, |store, _| store.library().songs().count())
    }

    fn workspace(&self, cx: &mut VisualTestContext) -> Entity<crate::songs::SongsWorkspace> {
        self.shell.read_with(cx, |shell, _| shell.songs().clone())
    }

    fn framed(&self, cx: &mut VisualTestContext) -> bool {
        self.shell.read_with(cx, |shell, _| shell.has_frame())
    }

    fn toasts(&self, cx: &mut VisualTestContext) -> Vec<String> {
        crate::songs::lyrics::tests::toasts(&self.shell, cx)
    }

    /// Clicks the header's Import and waits for the files to be read.
    fn import_from_header(&self, cx: &mut VisualTestContext) {
        click(cx, "songs-import-header");
        let workspace = self.workspace(cx);
        wait(cx, |cx| !workspace.read_with(cx, |ws, _| ws.reading()));
    }

    /// As [Self::import_from_header], then lets the frame finish rising so
    /// recorded bounds are where it rests.
    fn import_settled(&self, cx: &mut VisualTestContext) {
        self.import_from_header(cx);
        for _ in 0..10 {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(60));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        }
    }

    fn finish(&self, cx: &mut VisualTestContext) {
        let outcome = cx.foreground_executor().block_test(self.slot.shutdown());
        assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.files);
    }
}

const GLORIA: &str = "---\ntitle: Gloria\nlanguage: English\n---\n[Verse]\nGlory to God\n";

fn open_shell<'a>(
    cx: &'a mut TestAppContext,
    tag: &str,
    seed: &[ImportDocument],
) -> (ShellRig, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    let dir = temp_dir(tag);
    let files = temp_dir(&format!("{tag}-files"));
    let (slot, store) = open_parish(cx, &dir);
    for (index, document) in seed.iter().enumerate() {
        let path = write(&files, &format!("seed-{index}.ordo"), &ordo_bytes(document));
        let staging = stage(cx, &store, vec![path]);
        let task = store.update(cx, |store, cx| {
            store.commit_import(&staging, Choices::default(), cx)
        });
        cx.foreground_executor().block_test(task).expect("seeded");
    }
    let (shell, cx) = crate::songs::choirs::tests::open_store(cx, store.clone());
    let picker = Rc::new(ScriptedPicker::default());
    let installed: Rc<dyn FilePicker> = picker.clone();
    shell.update(cx, |shell, cx| shell.set_picker(installed, cx));
    cx.run_until_parked();
    (
        ShellRig {
            shell,
            store,
            slot,
            dir,
            files,
            picker,
        },
        cx,
    )
}

fn pathbufs(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn route_goes_by_extension_case_insensitively() {
    use super::{Picked, route};
    assert_eq!(
        route(&pathbufs(&["a.ordo", "B.ORDO"])),
        Picked::Ordo(pathbufs(&["a.ordo", "B.ORDO"]))
    );
    assert_eq!(
        route(&pathbufs(&["song.MD"])),
        Picked::Markdown(PathBuf::from("song.MD"))
    );
    assert_eq!(
        route(&pathbufs(&["song.txt"])),
        Picked::Markdown(PathBuf::from("song.txt"))
    );
    for refused in [
        &["a.md", "b.md"][..],
        &["a.md", "b.ordo"],
        &["a.ordo", "b.txt"],
        &["deck.pptx"],
        &["deck.ppt"],
        &["noextension"],
        &[],
    ] {
        assert_eq!(route(&pathbufs(refused)), Picked::Refused, "{refused:?}");
    }
}

#[gpui::test]
fn ac1_import_ordo_is_available_in_the_empty_state_and_the_songs_header(cx: &mut TestAppContext) {
    let (rig, cx) = open_shell(cx, "wp11-entry", &[]);
    assert!(present(cx, "songs-empty"));
    assert!(present(cx, "songs-import") && present(cx, "songs-import-header"));
    // Neither says "arrives in a later build": both ask for files.
    click(cx, "songs-import");
    assert_eq!(rig.picker.asked.get(), 1);
    assert_eq!(rig.picker.multiple.get(), Some(true));
    click(cx, "songs-import-header");
    assert_eq!(rig.picker.asked.get(), 2);
    assert!(!rig.framed(cx));
    // A populated library keeps the header entry.
    rig.finish(cx);
    let (rig, cx) = open_shell(cx, "wp11-entry-full", &tinapay_seed());
    assert!(!present(cx, "songs-import"));
    click(cx, "songs-import-header");
    assert_eq!(rig.picker.asked.get(), 1);
    rig.finish(cx);
}

#[gpui::test]
fn ac1_several_ordo_files_open_one_review(cx: &mut TestAppContext) {
    let (rig, cx) = open_shell(cx, "wp11-several", &[]);
    let a = rig.file("a.ordo", &ordo_bytes(&doc(1, 0x11, "Alleluia", "Sing")));
    let b = rig.file("B.ORDO", &ordo_bytes(&doc(2, 0x12, "Benedictus", "Bless")));
    rig.answer(vec![a, b]);
    rig.import_settled(cx);
    assert!(rig.framed(cx));
    let review = rig
        .shell
        .read_with(cx, |shell, _| shell.import_review().cloned())
        .expect("one review");
    assert_eq!(
        review.read_with(cx, |review, _| review.documents().len()),
        2
    );
    assert!(present(cx, "import-doc-0") && present(cx, "import-doc-1"));
    // The review covers the workspace: no Songs list pane, no rail.
    assert!(!present(cx, "songs-list-pane"));
    assert_eq!(
        rig.songs(cx),
        0,
        "nothing is written before Import approved"
    );
    assert!(rig.toasts(cx).is_empty());
    rig.finish(cx);
}

#[gpui::test]
fn ac3_one_markdown_or_text_file_opens_the_draft(cx: &mut TestAppContext) {
    for (name, tag) in [("gloria.md", "wp11-md"), ("gloria.TXT", "wp11-txt")] {
        let (rig, cx) = open_shell(cx, tag, &[]);
        let path = rig.file(name, GLORIA.as_bytes());
        rig.answer(vec![path]);
        rig.import_settled(cx);
        let draft = rig
            .shell
            .read_with(cx, |shell, _| shell.markdown_draft().cloned())
            .expect("the draft opens");
        draft.read_with(cx, |draft, cx| {
            assert_eq!(draft.parsed().document.song.title, "Gloria");
            assert_eq!(draft.source().value(cx).as_ref(), GLORIA);
        });
        assert!(present(cx, "markdown-source") && present(cx, "button-Save song"));
        assert!(
            rig.shell
                .read_with(cx, |shell, _| shell.import_review().is_none())
        );
        assert_eq!(
            rig.songs(cx),
            0,
            "{name}: nothing is saved before Save song"
        );
        rig.finish(cx);
    }
}

#[gpui::test]
fn ac1_a_mixed_or_powerpoint_selection_is_refused_with_a_toast_and_nothing_opens(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = open_shell(cx, "wp11-refuse", &[]);
    let ordo = rig.file("a.ordo", &ordo_bytes(&doc(1, 0x11, "Alleluia", "Sing")));
    let md = rig.file("one.md", GLORIA.as_bytes());
    let other = rig.file("two.md", GLORIA.as_bytes());
    let deck = rig.file("deck.pptx", b"not a song");
    let legacy = rig.file("deck.ppt", b"not a song");
    for (round, selection) in [
        vec![ordo, md.clone()],
        vec![md, other],
        vec![deck],
        vec![legacy],
    ]
    .into_iter()
    .enumerate()
    {
        rig.answer(selection);
        rig.import_settled(cx);
        cx.run_until_parked();
        assert!(!rig.framed(cx), "round {round}");
        assert_eq!(rig.songs(cx), 0, "round {round}");
        assert_eq!(
            rig.toasts(cx),
            ["Choose .ordo files, or one Markdown (.md or .txt) song."],
            "round {round}"
        );
        assert!(present(cx, "songs-empty"), "round {round}");
    }
    rig.finish(cx);
}

#[gpui::test]
fn ac1_cancelling_the_picker_changes_nothing(cx: &mut TestAppContext) {
    let (rig, cx) = open_shell(cx, "wp11-cancel-pick", &tinapay_seed());
    let before = rig.workspace(cx).read_with(cx, |ws, _| ws.selected());
    // The picker answers None, as a cancelled dialog does; an empty answer is
    // the same.
    rig.import_settled(cx);
    rig.picker.answers.borrow_mut().push(Some(Vec::new()));
    rig.import_settled(cx);
    assert_eq!(rig.picker.asked.get(), 2);
    assert!(!rig.framed(cx));
    assert!(rig.toasts(cx).is_empty());
    assert_eq!(rig.songs(cx), 1);
    assert_eq!(
        rig.workspace(cx).read_with(cx, |ws, _| ws.selected()),
        before
    );
    // And the entry still works afterwards.
    let path = rig.file("a.ordo", &ordo_bytes(&doc(2, 0x12, "Benedictus", "Bless")));
    rig.answer(vec![path]);
    rig.import_settled(cx);
    assert!(rig.framed(cx));
    rig.finish(cx);
}

#[gpui::test]
fn the_review_rises_and_cancel_returns_focus_to_import(cx: &mut TestAppContext) {
    let (rig, cx) = open_shell(cx, "wp11-rise", &[]);
    let path = rig.file("a.ordo", &ordo_bytes(&doc(1, 0x11, "Alleluia", "Sing")));
    rig.answer(vec![path]);
    rig.import_from_header(cx);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let frame = cx.debug_bounds("focused-frame").expect("the frame");
    let page = cx.debug_bounds("focused-frame-page").expect("the page");
    assert!(
        page.origin.y > frame.origin.y,
        "the frame starts below its resting place and rises"
    );
    // After the slideover timing it is at rest.
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(
            (ui::tokens::motion::STANDARD_DURATION_MS * 2).into(),
        ));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let page = cx.debug_bounds("focused-frame-page").expect("the page");
    assert_eq!(page.origin.y, frame.origin.y);
    click(cx, "button-Cancel import");
    assert!(!rig.framed(cx));
    assert!(present(cx, "songs-list-pane"));
    let focus = rig
        .workspace(cx)
        .read_with(cx, |ws, _| ws.import_focus().clone());
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.update(|window, _| focus.is_focused(window)));
    assert_eq!(rig.songs(cx), 0);
    rig.finish(cx);
}

#[gpui::test]
fn an_approved_import_returns_to_songs_with_the_first_created_song_selected(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = open_shell(cx, "wp11-commit", &[]);
    let a = rig.file("a.ordo", &ordo_bytes(&doc(1, 0x11, "Alleluia", "Sing")));
    let b = rig.file("b.ordo", &ordo_bytes(&doc(2, 0x12, "Benedictus", "Bless")));
    rig.answer(vec![a, b]);
    rig.import_settled(cx);
    click(cx, "button-Import approved");
    let shell = rig.shell.clone();
    wait(cx, |cx| !shell.read_with(cx, |shell, _| shell.has_frame()));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(rig.songs(cx), 2);
    assert_eq!(
        rig.toasts(cx),
        ["Imported the approved changes in one update."]
    );
    let workspace = rig.workspace(cx);
    let selected = workspace
        .read_with(cx, |ws, _| ws.selected())
        .expect("selected");
    assert_eq!(selected, song_id(1));
    let focused = cx.update(|window, cx| workspace.read(cx).row_focus().is_focused(window));
    assert!(focused, "the selected Song's row holds focus");
    rig.finish(cx);
}

#[gpui::test]
fn reading_files_shows_a_named_progress_indicator(cx: &mut TestAppContext) {
    let (rig, cx) = open_shell(cx, "wp11-reading", &[]);
    let path = rig.file("a.ordo", &ordo_bytes(&doc(1, 0x11, "Alleluia", "Sing")));
    rig.answer(vec![path]);
    let workspace = rig.workspace(cx);
    cx.update(|window, cx| workspace.update(cx, |ws, cx| ws.import(window, cx)));
    // Step the foreground until the read is under way, before it finishes.
    let mut seen = false;
    for _ in 0..200 {
        if workspace.read_with(cx, |ws, _| ws.reading()) {
            seen = true;
            break;
        }
        cx.executor().tick();
    }
    assert!(seen, "the indicator is up while the files are read");
    cx.update(|window, _| window.refresh());
    let role = cx.debug_bounds("songs-reading");
    assert!(role.is_some(), "the named indicator is drawn");
    let named = cx.update(|_, cx| {
        let _ = cx;
        super::super::library::READING_FILES
    });
    assert_eq!(named, "Reading files\u{2026}");
    wait(cx, |cx| !workspace.read_with(cx, |ws, _| ws.reading()));
    assert!(!present(cx, "songs-reading"));
    assert!(rig.framed(cx));
    rig.finish(cx);
}

#[gpui::test]
fn saving_a_draft_swaps_in_the_ordinary_editor_and_discarding_returns_to_import(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = open_shell(cx, "wp11-draft", &[]);
    let path = rig.file("gloria.md", GLORIA.as_bytes());
    rig.answer(vec![path.clone()]);
    rig.import_settled(cx);
    click(cx, "button-Discard draft");
    assert!(!rig.framed(cx), "an untouched draft closes at once");
    assert_eq!(rig.songs(cx), 0);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let focus = rig
        .workspace(cx)
        .read_with(cx, |ws, _| ws.import_focus().clone());
    assert!(cx.update(|window, _| focus.is_focused(window)));
    rig.answer(vec![path]);
    rig.import_settled(cx);
    click(cx, "button-Save song");
    let shell = rig.shell.clone();
    wait(cx, |cx| {
        shell.read_with(cx, |shell, _| shell.editing_arrangement().is_some())
    });
    assert!(!rig.framed(cx));
    assert_eq!(rig.songs(cx), 1);
    assert_eq!(rig.toasts(cx), ["Saved Gloria."]);
    rig.finish(cx);
}
