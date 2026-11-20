//! Behavior of the History slideover, its nested comparison and the
//! whole-entity restore, through the shell's overlay layer over a real
//! temporary parish. Times are read from the clock the sessions were written
//! with, so the row text that depends on it is asserted structurally; the
//! day and time formats have their own fixed-clock tests.

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use domain::{
    identity::{ArrangementId, EntityId, HistorySessionId, OccurrenceId, SectionId, SongId},
    revisions::{RevisionEntity, RevisionKind},
    song_library::{ArrangementLyrics, OccurrenceDraft},
};
use gpui::{
    Entity, Focusable as _, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext,
};
use storage::SongContent;
use time::{Date, Month, UtcOffset};

use super::{AfterRestore, HistoryTarget, HistoryView, Load, open, time as when};
use crate::{
    edit_session::SaveState,
    shell::ShellView,
    songs::{
        LibraryStore, StorageSlot,
        choirs::tests::{entity, open_store, present, wait},
        lyrics::{
            ArrangementEditor, TaskTab,
            tests::{dialog, in_memory, open_editor, parish_full, save_state, toasts},
        },
        new_song::tests::{click, finish, parish, settle},
    },
};

fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

fn has(cx: &mut VisualTestContext, selector: &str) -> bool {
    present(cx, leak(selector.to_owned()))
}

fn press(cx: &mut VisualTestContext, selector: &str) {
    click(cx, leak(selector.to_owned()));
}

struct Seed {
    dir: PathBuf,
    slot: StorageSlot,
    store: Entity<LibraryStore>,
    id: ArrangementId,
    song: SongId,
    /// The sections' lyrics right after the first edit (session A).
    after_a: Vec<String>,
}

fn session(byte: u8) -> HistorySessionId {
    HistorySessionId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

fn edit(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    by: u8,
    change: impl FnOnce(&mut ArrangementLyrics),
) {
    let mut lyrics = store.read_with(cx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("arrangement")
            .lyrics()
    });
    change(&mut lyrics);
    let task = store.update(cx, |store, cx| {
        store.save_arrangement_lyrics(id, lyrics, session(by), cx)
    });
    assert_eq!(cx.foreground_executor().block_test(task), Ok(id));
}

fn texts<C: gpui::AppContext>(
    cx: &C,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
) -> Vec<String> {
    store.read_with(cx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("arrangement")
            .lyrics()
            .sections
            .iter()
            .map(|section| section.lyrics.clone())
            .collect()
    })
}

/// A parish whose Arrangement "Cebuano SATB" has four History sessions, newest
/// first: C approves Verse 1, B adds a Chorus and edits Verse 1 (two
/// revisions), A first edits Verse 1, and the creation event.
fn seeded(cx: &mut TestAppContext, tag: &str) -> Seed {
    let (dir, slot, store, ids) = parish(cx, tag);
    let id = store.read_with(cx, |store, _| {
        store
            .library()
            .arrangements()
            .find(|arrangement| arrangement.details().title.as_deref() == Some("Cebuano SATB"))
            .expect("the seeded Arrangement")
            .id()
    });
    let verse = store.read_with(cx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("a")
            .lyrics()
            .sections[0]
            .id
    });
    edit(cx, &store, id, 0xa1, |lyrics| {
        lyrics
            .set_section_lyrics(verse, "Tinapay sa langit\nDayandayan ang gugma".into())
            .expect("verse");
    });
    let after_a = texts(cx, &store, id);
    let chorus = SectionId::from_entity(entity(0x70));
    edit(cx, &store, id, 0xb2, |lyrics| {
        lyrics.add_section(chorus, "Chorus").expect("chorus");
        lyrics
            .set_section_lyrics(chorus, "Alleluia".into())
            .expect("words");
        lyrics.sequence.push(OccurrenceDraft::new(
            OccurrenceId::from_entity(entity(0x71)),
            chorus,
        ));
    });
    edit(cx, &store, id, 0xb2, |lyrics| {
        lyrics
            .set_section_lyrics(
                verse,
                "Tinapay sa langit\nDayandayan ang gugma natong tanan".into(),
            )
            .expect("verse");
    });
    edit(cx, &store, id, 0xc3, |lyrics| {
        lyrics.approve_section(verse).expect("approve");
    });
    Seed {
        dir,
        slot,
        store,
        id,
        song: ids.exact,
        after_a,
    }
}

type Calls = Rc<RefCell<Vec<HistoryTarget>>>;

fn open_view(
    cx: &mut VisualTestContext,
    shell: &Entity<ShellView>,
    store: &Entity<LibraryStore>,
    target: HistoryTarget,
    calls: &Calls,
) -> Entity<HistoryView> {
    let host = shell.read_with(cx, |shell, _| shell.overlays().clone());
    let record = calls.clone();
    let after = AfterRestore::new(move |target, _, _| record.borrow_mut().push(target));
    let store = store.clone();
    let view = cx.update(|window, cx| open(target, store, host, after, window, cx));
    wait(cx, |cx| {
        view.read_with(cx, |view, _| matches!(view.load, Load::Ready))
    });
    settle(cx);
    view
}

fn depth(cx: &mut VisualTestContext, shell: &Entity<ShellView>) -> usize {
    let host = shell.read_with(cx, |shell, _| shell.overlays().clone());
    cx.update(|_, cx| host.depth(cx))
}

fn revisions(
    cx: &mut VisualTestContext,
    slot: &StorageSlot,
    id: ArrangementId,
) -> Vec<storage::HistoryRevision> {
    let future = slot
        .with(|storage| storage.history(RevisionEntity::Arrangement(id)))
        .expect("storage is open");
    cx.foreground_executor()
        .block_test(future)
        .expect("history")
}

fn compare(cx: &mut VisualTestContext, session: usize, revision: usize) {
    press(cx, &format!("history-revision-{session}-{revision}"));
    settle(cx);
}

/// The "when" the UI words for a stored revision, from its row.
fn stored_when(
    vcx: &mut VisualTestContext,
    seed: &Seed,
    view: &Entity<HistoryView>,
    session: usize,
    revision: usize,
) -> String {
    let id = view.read_with(vcx, |view, _| view.sessions[session].revisions[revision].id);
    let stored = revisions(vcx, &seed.slot, seed.id)
        .into_iter()
        .find(|stored| stored.record.id == id)
        .expect("stored");
    when::when(
        stored.record.created_at,
        ::time::OffsetDateTime::now_utc(),
        when::local_offset(),
    )
}

fn arrangement_target(seed: &Seed) -> HistoryTarget {
    HistoryTarget::Arrangement(seed.id)
}

#[gpui::test]
fn ac1_history_lists_sessions_newest_first_with_when_how_and_revision_count(
    cx: &mut TestAppContext,
) {
    let seed = seeded(cx, "t29-list");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);

    let rows = view.read_with(vcx, |view, _| view.sessions.clone());
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].what, "Edited lyrics \u{b7} 1 change");
    assert_eq!(rows[1].what, "Edited lyrics \u{b7} 2 changes");
    assert_eq!(rows[2].what, "Edited lyrics \u{b7} 1 change");
    assert_eq!(rows[3].what, "Created arrangement");
    assert!(
        rows[3].revisions[0].what.starts_with("Created with "),
        "{}",
        rows[3].revisions[0].what
    );
    assert_eq!(rows[3].how, "Creation");
    for row in &rows[..3] {
        assert_eq!(row.how, "Manual edit");
    }
    for row in &rows {
        assert!(row.when.starts_with("Today \u{b7} "), "{}", row.when);
    }
    assert_eq!(rows[1].revisions.len(), 2);
    assert_eq!(rows[0].revisions.len(), 1);
    for at in 0..4 {
        assert!(has(vcx, &format!("history-session-{at}")));
    }
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac1_expanding_a_session_shows_its_revisions_and_marks_current(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-expand");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);

    // The newest session opens with History, and only it.
    assert!(has(vcx, "history-revision-0-0"));
    assert!(has(vcx, "history-current"));
    assert!(!has(vcx, "history-revision-1-0"));

    // Down twice reaches the second session; Right opens it.
    vcx.simulate_keystrokes("down down right");
    vcx.run_until_parked();
    assert!(view.read_with(vcx, |view, _| view.expanded.contains(&1)));
    assert!(has(vcx, "history-revision-1-0") && has(vcx, "history-revision-1-1"));

    // A click on a session row opens and closes it too.
    press(vcx, "history-session-2");
    assert!(has(vcx, "history-revision-2-0"));
    press(vcx, "history-session-2");
    assert!(!has(vcx, "history-revision-2-0"));

    let rows = view.read_with(vcx, |view, _| view.sessions.clone());
    let currents = rows
        .iter()
        .flat_map(|row| &row.revisions)
        .filter(|row| row.current)
        .count();
    assert_eq!(currents, 1, "only the head reads Current");
    assert!(rows[0].revisions[0].current);
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac1_an_approval_only_revision_reads_approved_pagination(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-approval");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);

    let rows = view.read_with(vcx, |view, _| view.sessions.clone());
    assert_eq!(rows[0].revisions[0].what, "Approved pagination");
    finish(vcx, &seed.slot, &seed.dir);
}

#[test]
fn ac1_session_times_read_today_yesterday_and_dates() {
    let manila = UtcOffset::from_hms(8, 0, 0).expect("offset");
    let at = |day: u8, hour: u8, minute: u8| {
        let moment = Date::from_calendar_date(2026, Month::August, day)
            .expect("date")
            .with_hms(hour, minute, 0)
            .expect("time")
            .assume_offset(manila);
        domain::identity::Timestamp::from_millis(moment.unix_timestamp() * 1_000)
    };
    let now = Date::from_calendar_date(2026, Month::August, 20)
        .expect("date")
        .with_hms(12, 0, 0)
        .expect("time")
        .assume_offset(manila);
    assert_eq!(
        when::session_when(at(20, 10, 37), at(20, 10, 44), now, manila),
        "Today \u{b7} 10:37\u{2013}10:44"
    );
    assert_eq!(
        when::session_when(at(19, 16, 12), at(19, 16, 12), now, manila),
        "Yesterday \u{b7} 16:12"
    );
    assert_eq!(
        when::session_when(at(14, 9, 8), at(14, 9, 8), now, manila),
        "14 Aug 2026 \u{b7} 09:08"
    );
    assert_eq!(when::when(at(20, 10, 44), now, manila), "10:44");
    assert_eq!(when::when(at(19, 16, 12), now, manila), "Yesterday 16:12");
}

#[gpui::test]
fn ac2_selecting_a_revision_opens_a_nested_comparison_and_history_recedes(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-nested");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    assert_eq!(depth(vcx, &shell), 1);

    // The head opens nothing.
    press(vcx, "history-revision-0-0");
    settle(vcx);
    assert_eq!(depth(vcx, &shell), 1);

    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    assert_eq!(depth(vcx, &shell), 2);
    assert!(has(vcx, "compare-body"));
    assert_eq!(view.read_with(vcx, |view, _| view.selected), Some((2, 0)));
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac2_arrangement_comparison_has_overview_lyrics_sequence_and_slides(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-tabs");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);

    let names = ["Overview", "Lyrics", "Sequence", "Slides"];
    for at in 0..4 {
        assert!(has(vcx, &format!("tab-{at}")), "tab {at}");
    }
    assert!(!has(vcx, "tab-4"));
    assert!(has(vcx, "compare-current") && has(vcx, "compare-revision"));
    let compare = view.read_with(vcx, |view, _| view.compare.clone().expect("open"));
    for (at, name) in names.iter().enumerate() {
        press(vcx, &format!("tab-{at}"));
        assert_eq!(compare.read_with(vcx, |view, cx| view.tab(cx)), *name);
    }
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac2_comparison_marks_changed_lines_and_occurrences_only(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-marks");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);

    press(vcx, "tab-1");
    assert!(has(vcx, "compare-current-line-0-0"), "an unchanged line");
    assert!(!has(vcx, "compare-current-line-0-0-changed"));
    assert!(!has(vcx, "compare-revision-line-0-0-changed"));
    assert!(has(vcx, "compare-current-line-0-1-changed"));
    assert!(has(vcx, "compare-revision-line-0-1-changed"));
    assert!(
        has(vcx, "compare-current-line-1-0-changed"),
        "the new Chorus"
    );

    press(vcx, "tab-2");
    assert!(has(vcx, "compare-current-seq-0"));
    assert!(!has(vcx, "compare-current-seq-0-changed"));
    assert!(has(vcx, "compare-current-seq-1-changed"));
    assert!(has(vcx, "compare-revision-seq-0"));
    assert!(!has(vcx, "compare-revision-seq-1"));
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac2_comparison_is_read_only(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-readonly");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    let before = texts(vcx, &seed.store, seed.id);
    let count = revisions(vcx, &seed.slot, seed.id).len();

    for at in 0..4 {
        press(vcx, &format!("tab-{at}"));
        for control in [
            "focused-editor",
            "arrangement-lyrics",
            "section-add",
            "button-Add section",
            "button-Save",
        ] {
            assert!(!has(vcx, control), "{control} on tab {at}");
        }
    }
    vcx.simulate_input("typed words");
    vcx.run_until_parked();
    assert_eq!(texts(vcx, &seed.store, seed.id), before);
    assert_eq!(revisions(vcx, &seed.slot, seed.id).len(), count);
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac2_slides_view_shows_canonical_frames_composed_off_the_gpui_thread(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-slides");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    press(vcx, "tab-3");

    // Frames arrive from the background executor, not in the same turn.
    wait(vcx, |cx| {
        cx.update(|window, _| window.refresh());
        present(cx, "compare-current-slide-0") && present(cx, "compare-revision-slide-0")
    });
    let frames = view.read_with(vcx, |view, _| view.frames.clone());
    assert!(frames.read_with(vcx, |frames, _| frames.held()) > 0);
    assert_eq!(frames.read_with(vcx, |frames, _| frames.pending()), 0);
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac2_a_legacy_revision_shows_no_pagination_state_difference(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-legacy");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    // A revision written before section states were recorded.
    view.update(vcx, |view, _| {
        let mut all = view.revisions.clone();
        for revision in &mut all {
            revision.pagination_recorded = false;
        }
        view.revisions = all;
    });
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    press(vcx, "tab-1");
    assert!(
        !has(vcx, "compare-revision-line-0-0-changed")
            && !has(vcx, "compare-current-line-0-0-changed"),
        "a state-only difference marks no line"
    );

    let compare = view.read_with(vcx, |view, _| view.compare.clone().expect("open"));
    let (current, legacy) = compare.read_with(vcx, |view, _| view.lyrics_heads());
    assert!(
        current.iter().all(|head| head.contains(" \u{b7} ")),
        "the current side knows its states: {current:?}"
    );
    assert!(
        legacy.iter().all(|head| !head.contains(" \u{b7} ")),
        "the legacy side shows no state: {legacy:?}"
    );
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac2_song_comparison_has_overview_only(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-song");
    // Two Song edits written straight to storage; memory keeps the original.
    let slot = seed.slot.clone();
    let song = seed.song;
    let future = slot
        .with(|storage| storage.latest_revision(RevisionEntity::Song(song)))
        .expect("open");
    let mut parent = cx.foreground_executor().block_test(future).expect("latest");
    for (n, title) in [
        (0x91u8, "Tinapay sa Langit (alt)"),
        (0x92, "Tinapay sa Langit (alt 2)"),
    ] {
        let content = SongContent {
            title: title.to_owned(),
            aliases: Vec::new(),
            original_attribution: Some("Fr. Manoling Francisco".to_owned()),
            provenance: None,
            rights_notes: None,
            liturgical_moments: Vec::new(),
            occasions: Vec::new(),
        };
        let future = slot
            .with(|storage| {
                storage.commit_song_edit(
                    song,
                    content,
                    RevisionKind::Edit,
                    Some(session(n)),
                    parent,
                )
            })
            .expect("open");
        let (_, record) = cx.foreground_executor().block_test(future).expect("commit");
        parent = record.id;
    }
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = open_view(vcx, &shell, &seed.store, HistoryTarget::Song(song), &calls);
    press(vcx, "history-session-1");
    compare(vcx, 1, 0);

    assert!(has(vcx, "compare-body"));
    let compare = _view.read_with(vcx, |view, _| view.compare.clone().expect("open"));
    assert_eq!(compare.read_with(vcx, |view, cx| view.tab(cx)), "Overview");
    assert!(
        !has(vcx, "compare-current-line-0-0"),
        "a Song has no Lyrics"
    );
    assert!(has(vcx, "compare-current-Title-changed"));
    assert!(has(vcx, "compare-revision-Title-changed"));
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac2_back_returns_to_history_with_expansion_and_selection_kept(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-back");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    press(vcx, "button-Back to History");
    settle(vcx);

    assert_eq!(depth(vcx, &shell), 1);
    let (expanded, selected) =
        view.read_with(vcx, |view, _| (view.expanded.clone(), view.selected));
    assert!(expanded.contains(&0) && expanded.contains(&2));
    assert_eq!(selected, Some((2, 0)));
    assert!(has(vcx, "history-revision-2-0"));
    finish(vcx, &seed.slot, &seed.dir);
}

fn to_confirmation(
    vcx: &mut VisualTestContext,
    shell: &Entity<ShellView>,
    seed: &Seed,
    calls: &Calls,
) -> Entity<HistoryView> {
    let view = open_view(vcx, shell, &seed.store, arrangement_target(seed), calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    press(vcx, "button-Restore this revision");
    settle(vcx);
    view
}

#[gpui::test]
fn ac3_restore_this_revision_turns_the_footer_into_the_confirmation(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-confirm");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = to_confirmation(vcx, &shell, &seed, &calls);

    assert!(has(vcx, "two-stage-message"));
    assert!(has(vcx, "button-Keep current"));
    assert!(has(vcx, "button-Restore entire Arrangement"));
    assert!(!has(vcx, "button-Restore this revision"));
    let (lead, message) = view.read_with(vcx, |view, _| {
        view.stage
            .as_ref()
            .and_then(|stage| stage.confirmation_text())
            .expect("set")
    });
    let at = stored_when(vcx, &seed, &view, 2, 0);
    assert_eq!(lead, format!("Restore the whole Arrangement to {at}?"));
    assert_eq!(
        message,
        "Used by Koro Sto. Ni\u{f1}o. Mass-local copies and past Presentation snapshots don\u{2019}t change. It becomes a new revision, so nothing in History is lost."
    );
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac3_keep_current_returns_the_footer_and_changes_nothing(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-keep");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = to_confirmation(vcx, &shell, &seed, &calls);
    let before = texts(vcx, &seed.store, seed.id);
    let count = revisions(vcx, &seed.slot, seed.id).len();

    press(vcx, "button-Keep current");
    settle(vcx);
    assert!(has(vcx, "button-Restore this revision"));
    assert!(!has(vcx, "button-Restore entire Arrangement"));
    assert_eq!(depth(vcx, &shell), 2);
    assert_eq!(texts(vcx, &seed.store, seed.id), before);
    assert_eq!(revisions(vcx, &seed.slot, seed.id).len(), count);
    assert!(calls.borrow().is_empty());
    finish(vcx, &seed.slot, &seed.dir);
}

fn wait_for_revisions(
    vcx: &mut VisualTestContext,
    seed: &Seed,
    expected: usize,
) -> Vec<storage::HistoryRevision> {
    wait(vcx, |cx| {
        revisions(cx, &seed.slot, seed.id).len() == expected
    });
    settle(vcx);
    revisions(vcx, &seed.slot, seed.id)
}

#[gpui::test]
fn ac3_restore_entire_arrangement_writes_one_later_restoration_revision(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-restore");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = to_confirmation(vcx, &shell, &seed, &calls);
    let count = revisions(vcx, &seed.slot, seed.id).len();

    press(vcx, "button-Restore entire Arrangement");
    let after = wait_for_revisions(vcx, &seed, count + 1);

    assert_eq!(
        after.last().expect("head").record.kind,
        RevisionKind::Restoration
    );
    assert_eq!(texts(vcx, &seed.store, seed.id), seed.after_a);
    assert_eq!(depth(vcx, &shell), 0, "both surfaces closed");
    assert!(has(vcx, "button-Undo"), "the confirmation offers Undo");
    assert_eq!(calls.borrow().len(), 1);
    assert!(matches!(calls.borrow()[0], HistoryTarget::Arrangement(id) if id == seed.id));
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac3_undo_after_restore_is_another_later_revision(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-undo");
    let before = texts(cx, &seed.store, seed.id);
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = to_confirmation(vcx, &shell, &seed, &calls);
    let count = revisions(vcx, &seed.slot, seed.id).len();
    press(vcx, "button-Restore entire Arrangement");
    wait_for_revisions(vcx, &seed, count + 1);

    press(vcx, "button-Undo");
    let after = wait_for_revisions(vcx, &seed, count + 2);
    assert_eq!(
        after.last().expect("head").record.kind,
        RevisionKind::Restoration
    );
    assert_eq!(texts(vcx, &seed.store, seed.id), before);
    assert_eq!(
        calls.borrow().len(),
        2,
        "the Undo calls after_restore again, as the restore did"
    );
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac3_no_partial_restore_control_exists(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-whole");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);

    for at in 0..4 {
        press(vcx, &format!("tab-{at}"));
        assert!(has(vcx, "button-Restore this revision"), "tab {at}");
        for partial in [
            "button-Restore section",
            "button-Restore this section",
            "button-Restore lyrics",
            "button-Restore this line",
            "button-Restore sequence",
            "button-Restore slide",
        ] {
            assert!(!has(vcx, partial), "{partial} on tab {at}");
        }
    }
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac3_archived_entities_offer_no_restore_this_revision(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-archived");
    let song = seed.song;
    let task = seed
        .store
        .update(cx, |store, cx| store.archive_song(song, cx));
    assert_eq!(cx.foreground_executor().block_test(task), Ok(song));
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);

    assert!(
        has(vcx, "compare-body"),
        "an archived entity can still be read"
    );
    assert!(has(vcx, "button-Back to History"));
    assert!(!has(vcx, "button-Restore this revision"));
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn focus_stays_in_the_top_slideover_and_returns_to_its_invoker(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-focus");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    let list_focused = |vcx: &mut VisualTestContext| {
        let focus = view.read_with(vcx, |view, _| view.focus.clone());
        vcx.update(|window, _| focus.is_focused(window))
    };
    assert!(list_focused(vcx), "History takes focus on its list");

    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    assert!(!list_focused(vcx), "the receded History keeps no focus");

    press(vcx, "button-Back to History");
    settle(vcx);
    assert!(list_focused(vcx), "Back returns focus to the list");

    vcx.simulate_keystrokes("escape");
    settle(vcx);
    assert_eq!(depth(vcx, &shell), 0);
    assert!(!list_focused(vcx));
    let invoker = shell.read_with(vcx, |shell, cx| shell.focus_handle(cx));
    assert!(
        vcx.update(|window, cx| invoker.contains_focused(window, cx)),
        "focus returns to the shell that opened History"
    );
    finish(vcx, &seed.slot, &seed.dir);
}

// ---- WP7: the header History actions (T2.9 AC1) ----

const HISTORY_BLOCKED: &str =
    "Couldn\u{2019}t save, so History didn\u{2019}t open. Retry save first.";

fn history_is_open(cx: &mut VisualTestContext, shell: &Entity<ShellView>) -> bool {
    dialog(shell, cx).as_deref() == Some("History")
}

fn wait_for_history(cx: &mut VisualTestContext, shell: &Entity<ShellView>) {
    wait(cx, |cx| history_is_open(cx, shell));
    wait(cx, |cx| has(cx, "history-session-0"));
    settle(cx);
}

fn type_text(editor: &Entity<ArrangementEditor>, text: &str, cx: &mut VisualTestContext) {
    let session = editor
        .read_with(cx, |editor, _| editor.session().cloned())
        .expect("an editable Arrangement has a session");
    session.update(cx, |session, cx| {
        session.text_edited(|content| content.sections[0].lyrics = text.to_owned(), cx);
    });
    cx.run_until_parked();
}

fn first_lyrics(editor: &Entity<ArrangementEditor>, cx: &mut VisualTestContext) -> String {
    editor.read_with(cx, |editor, cx| {
        editor.content(cx).expect("content").sections[0]
            .lyrics
            .clone()
    })
}

fn select_song(shell: &Entity<ShellView>, song: SongId, cx: &mut VisualTestContext) {
    let songs = shell.read_with(cx, |shell, _| shell.songs().clone());
    songs.update(cx, |songs, cx| {
        songs.refresh(cx);
        songs.select(song, cx);
    });
    settle(cx);
}

#[gpui::test]
fn ac1_song_detail_header_offers_history_and_opens_a_full_app_slideover(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-song-header");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    select_song(&shell, seed.song, vcx);
    assert!(has(vcx, "button-History"), "the Song header offers History");
    assert!(!history_is_open(vcx, &shell));

    press(vcx, "button-History");
    wait_for_history(vcx, &shell);
    assert_eq!(depth(vcx, &shell), 1);
    assert!(has(vcx, "overlay-body-History"));
    // It lives on the shell's overlay layer, so its scrim covers the whole app,
    // the rail included.
    let scrim = vcx.debug_bounds("overlay-scrim").expect("scrim");
    assert_eq!(f32::from(scrim.size.width), 1920.0);
    assert_eq!(f32::from(scrim.size.height), 1080.0);
    assert_eq!(f32::from(scrim.origin.x), 0.0);
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac1_arrangement_header_offers_history_beside_arrangement_details(cx: &mut TestAppContext) {
    let (shell, _editor, _id, vcx) = in_memory(cx, false);
    let details = vcx
        .debug_bounds("button-Arrangement details")
        .expect("details");
    let history = vcx.debug_bounds("button-History").expect("history");
    assert!(history.left() >= details.right(), "History follows details");
    assert!(
        (f32::from(history.center().y) - f32::from(details.center().y)).abs() < 2.0,
        "one header row"
    );
    assert!(details.size.width > history.size.width);
    let wide_details = f32::from(details.size.width);

    // Where only the editor stays, both become icon buttons with the same names.
    vcx.simulate_resize(gpui::size(gpui::px(800.0), gpui::px(600.0)));
    settle(vcx);
    let details = vcx
        .debug_bounds("button-Arrangement details")
        .expect("details");
    let history = vcx.debug_bounds("button-History").expect("history");
    assert!(f32::from(details.size.width) < wide_details, "icon only");
    assert!(
        (f32::from(details.size.width) - f32::from(history.size.width)).abs() < 1.0,
        "two square icon buttons"
    );
    assert!(history.left() >= details.right());
    assert!(!history_is_open(vcx, &shell));
}

#[gpui::test]
fn ac1_history_is_offered_for_archived_arrangements_but_details_is_not(cx: &mut TestAppContext) {
    let (shell, _editor, _id, vcx) = in_memory(cx, true);
    assert!(has(vcx, "button-History"));
    assert!(!has(vcx, "button-Arrangement details"));
    press(vcx, "button-History");
    wait(vcx, |cx| history_is_open(cx, &shell));
}

#[gpui::test]
fn ac1_opening_history_flushes_pending_lyrics_first(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-flush");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let editor = open_editor(&shell, seed.id, vcx);
    let before = revisions(vcx, &seed.slot, seed.id).len();
    type_text(&editor, "Words typed just before History", vcx);
    assert_eq!(save_state(&editor, vcx), Some(SaveState::Saving));

    press(vcx, "button-History");
    wait_for_history(vcx, &shell);

    assert_eq!(
        texts(vcx, &seed.store, seed.id)[0],
        "Words typed just before History"
    );
    let after = revisions(vcx, &seed.slot, seed.id);
    assert_eq!(after.len(), before + 1, "one revision for the flushed text");
    assert_eq!(after.last().expect("head").record.kind, RevisionKind::Edit);
    assert_eq!(save_state(&editor, vcx), Some(SaveState::Saved));
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac1_a_failed_flush_keeps_the_edit_and_does_not_open_history(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    type_text(&editor, "Unsaved words", vcx);

    press(vcx, "button-History");
    settle(vcx);

    assert!(!history_is_open(vcx, &shell));
    assert_eq!(depth(vcx, &shell), 0, "no History and no dialog");
    assert_eq!(save_state(&editor, vcx), Some(SaveState::CouldntSave));
    assert!(has(vcx, "button-Retry save"), "the header keeps Retry save");
    assert_eq!(
        first_lyrics(&editor, vcx),
        "Unsaved words",
        "the edit stays"
    );
    assert_eq!(toasts(&shell, vcx), vec![HISTORY_BLOCKED.to_owned()]);

    // After a failed save the header already says so; History stays closed.
    press(vcx, "button-History");
    settle(vcx);
    assert!(!history_is_open(vcx, &shell));
    assert_eq!(first_lyrics(&editor, vcx), "Unsaved words");
}

#[gpui::test]
fn ac1_song_history_never_lists_arrangement_revisions(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-song-only");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let song_revisions = {
        let future = seed
            .slot
            .with(|storage| storage.history(RevisionEntity::Song(seed.song)))
            .expect("storage is open");
        vcx.foreground_executor()
            .block_test(future)
            .expect("history")
            .len()
    };
    assert_eq!(song_revisions, 1, "the Song was only created");
    assert_eq!(revisions(vcx, &seed.slot, seed.id).len(), 5);

    select_song(&shell, seed.song, vcx);
    press(vcx, "button-History");
    wait_for_history(vcx, &shell);
    assert!(has(vcx, "history-session-0"));
    assert!(
        !has(vcx, "history-session-1"),
        "the Arrangement's four sessions are not the Song's"
    );
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac1_the_editor_underneath_keeps_its_tab_selection_and_text(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t29-underneath");
    let editor = open_editor(&shell, id, vcx);

    // Lyrics tab with text that is still pending.
    type_text(&editor, "Kept words", vcx);
    press(vcx, "button-History");
    wait_for_history(vcx, &shell);
    vcx.simulate_keystrokes("escape");
    settle(vcx);
    assert!(!history_is_open(vcx, &shell));
    assert_eq!(first_lyrics(&editor, vcx), "Kept words");
    assert_eq!(
        editor.read_with(vcx, |editor, cx| editor.tab(cx)),
        TaskTab::Lyrics
    );

    // Sequence tab with a selected occurrence.
    press(vcx, "tab-1");
    settle(vcx);
    press(vcx, "sequence-name-1");
    let selected = editor.read_with(vcx, |editor, _| {
        editor
            .sequence_selection
            .map(|selection| selection.occurrence)
    });
    assert!(selected.is_some());
    press(vcx, "button-History");
    wait_for_history(vcx, &shell);
    vcx.simulate_keystrokes("escape");
    settle(vcx);
    assert!(
        shell.read_with(vcx, |shell, _| shell.arrangement_editor() == Some(&editor)),
        "the same editor stays under History"
    );
    assert_eq!(
        editor.read_with(vcx, |editor, cx| editor.tab(cx)),
        TaskTab::Sequence
    );
    assert_eq!(
        editor.read_with(vcx, |editor, _| {
            editor
                .sequence_selection
                .map(|selection| selection.occurrence)
        }),
        selected
    );
    assert_eq!(first_lyrics(&editor, vcx), "Kept words");
    finish(vcx, &slot, &dir);
}

/// Native keyboard activation is a key press, not an action dispatch, so it has
/// to arrive as the raw pair the platform would send.
fn press_key(cx: &mut VisualTestContext, key: &str) {
    let keystroke = Keystroke::parse(key).expect("a parseable key");
    cx.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(KeyUpEvent { keystroke });
    cx.run_until_parked();
}

#[gpui::test]
fn ac1_closing_history_returns_focus_to_its_button(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-return-focus");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let focused = |vcx: &mut VisualTestContext, handle: &gpui::FocusHandle| {
        vcx.update(|window, _| handle.is_focused(window))
    };

    // Song detail: closing hands focus back to the header's History button,
    // and Enter there opens History again.
    select_song(&shell, seed.song, vcx);
    let songs = shell.read_with(vcx, |shell, _| shell.songs().clone());
    let button = songs.read_with(vcx, |songs, _| songs.history_focus().clone());
    press(vcx, "button-History");
    wait_for_history(vcx, &shell);
    assert!(!focused(vcx, &button), "the slideover holds focus");
    vcx.simulate_keystrokes("escape");
    settle(vcx);
    assert!(!history_is_open(vcx, &shell));
    assert!(
        focused(vcx, &button),
        "focus returns to the Song History button"
    );
    assert_eq!(
        vcx.update(|window, cx| shell.read(cx).focused_region(window, cx)),
        Some("Main editor".into())
    );
    press_key(vcx, "enter");
    wait_for_history(vcx, &shell);
    vcx.simulate_keystrokes("escape");
    settle(vcx);

    // Arrangement header, the same way.
    let _editor = open_editor(&shell, seed.id, vcx);
    let button = shell.read_with(vcx, |shell, _| shell.history_focus().clone());
    press(vcx, "button-History");
    wait_for_history(vcx, &shell);
    assert!(!focused(vcx, &button), "the slideover holds focus");
    vcx.simulate_keystrokes("escape");
    settle(vcx);
    assert!(!history_is_open(vcx, &shell));
    assert!(
        focused(vcx, &button),
        "focus returns to the Arrangement History button"
    );
    assert_eq!(
        vcx.update(|window, cx| shell.read(cx).focused_region(window, cx)),
        Some("Workspace actions".into()),
        "the Arrangement header is the Workspace actions region"
    );
    press_key(vcx, "enter");
    wait_for_history(vcx, &shell);
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac1_after_a_restore_the_editor_reopens_on_the_same_tab_with_a_new_session(
    cx: &mut TestAppContext,
) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t29-reopen");
    let editor = open_editor(&shell, id, vcx);
    press(vcx, "tab-1");
    settle(vcx);
    let count = |vcx: &mut VisualTestContext| {
        let future = slot
            .with(|storage| storage.history(RevisionEntity::Arrangement(id)))
            .expect("storage is open");
        vcx.foreground_executor()
            .block_test(future)
            .expect("history")
            .len()
    };
    let before = count(vcx);
    let old_session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session")
        .entity_id();

    press(vcx, "button-History");
    wait_for_history(vcx, &shell);
    press(vcx, "history-session-1");
    compare(vcx, 1, 0);
    press(vcx, "button-Restore this revision");
    settle(vcx);
    press(vcx, "button-Restore entire Arrangement");
    wait(vcx, |vcx| count(vcx) == before + 1);
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| {
            shell.arrangement_editor().is_some_and(|now| *now != editor)
        })
    });
    settle(vcx);

    let reopened = shell.read_with(vcx, |shell, _| {
        shell.arrangement_editor().expect("editor").clone()
    });
    assert_eq!(
        reopened.read_with(vcx, |editor, cx| editor.tab(cx)),
        TaskTab::Sequence
    );
    let new_session = reopened
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session")
        .entity_id();
    assert_ne!(new_session, old_session, "a new session");
    let stored = texts(vcx, &_store, id)[0].clone();
    assert_eq!(first_lyrics(&reopened, vcx), stored, "the restored lyrics");
    reopened.update(vcx, |editor, cx| editor.undo(cx));
    settle(vcx);
    assert_eq!(
        first_lyrics(&reopened, vcx),
        stored,
        "Undo has nothing to undo"
    );
    assert_eq!(count(vcx), before + 1, "and writes nothing");
    let button = shell.read_with(vcx, |shell, _| shell.history_focus().clone());
    assert!(
        vcx.update(|window, _| button.is_focused(window)),
        "focus returns to the header History button"
    );
    finish(vcx, &slot, &dir);
}

// ---- Review fixes (T2.9b) ----

/// A second writer: one more lyric revision through the store, so the head
/// History was opened on is no longer the head.
fn write_more(vcx: &mut VisualTestContext, seed: &Seed, by: u8) {
    let mut lyrics = seed.store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangement(seed.id)
            .expect("arrangement")
            .lyrics()
    });
    let section = lyrics.sections[0].id;
    lyrics
        .set_section_lyrics(section, "Written by someone else".into())
        .expect("section");
    let task = seed.store.update(vcx, |store, cx| {
        store.save_arrangement_lyrics(seed.id, lyrics, session(by), cx)
    });
    assert_eq!(vcx.foreground_executor().block_test(task), Ok(seed.id));
}

const STALE: &str = "This changed since you opened History. Reopen it to restore.";

#[gpui::test]
fn a_history_read_failure_shows_the_refusal_and_escape_closes(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-read-fails");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let host = shell.read_with(vcx, |shell, _| shell.overlays().clone());
    let missing = ArrangementId::from_entity(EntityId::from_random_bytes([0x5a; 16]));
    let store = seed.store.clone();
    let after = AfterRestore::new(|_, _, _| {});
    let view = vcx.update(|window, cx| {
        open(
            HistoryTarget::Arrangement(missing),
            store,
            host,
            after,
            window,
            cx,
        )
    });
    wait(vcx, |cx| {
        view.read_with(cx, |view, _| matches!(view.load, Load::Failed(_)))
    });
    settle(vcx);

    assert!(
        has(vcx, "history-failed"),
        "the refusal is in the slideover"
    );
    let message = view.read_with(vcx, |view, _| match &view.load {
        Load::Failed(message) => message.to_string(),
        _ => String::new(),
    });
    assert_eq!(message, super::COULDNT_OPEN);
    assert!(toasts(&shell, vcx).is_empty(), "no toast says it twice");
    assert_eq!(depth(vcx, &shell), 1);

    vcx.simulate_keystrokes("escape");
    settle(vcx);
    assert_eq!(depth(vcx, &shell), 0, "Escape closes the failed History");
    let invoker = shell.read_with(vcx, |shell, cx| shell.focus_handle(cx));
    assert!(
        vcx.update(|window, cx| invoker.contains_focused(window, cx)),
        "focus is back on the shell"
    );
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac3_undo_after_restore_is_refused_while_a_save_failed(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-undo-blocked");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let _editor = open_editor(&shell, seed.id, vcx);
    let count = revisions(vcx, &seed.slot, seed.id).len();

    press(vcx, "button-History");
    wait_for_history(vcx, &shell);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    press(vcx, "button-Restore this revision");
    settle(vcx);
    press(vcx, "button-Restore entire Arrangement");
    wait_for_revisions(vcx, &seed, count + 1);
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| {
            shell
                .arrangement_editor()
                .is_some_and(|now| *now != _editor)
        })
    });
    settle(vcx);
    let reopened = shell.read_with(vcx, |shell, _| {
        shell.arrangement_editor().expect("editor").clone()
    });

    let future = seed
        .slot
        .with(|storage| storage.inject_write_failure(1))
        .expect("storage is open");
    vcx.foreground_executor().block_test(future).expect("armed");
    type_text(&reopened, "Typed after the restore", vcx);
    press(vcx, "button-Undo");
    settle(vcx);

    assert_eq!(first_lyrics(&reopened, vcx), "Typed after the restore");
    assert_eq!(revisions(vcx, &seed.slot, seed.id).len(), count + 1);
    assert_eq!(
        toasts(&shell, vcx).last().map(String::as_str),
        Some(super::UNDO_BLOCKED)
    );
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac3_a_refused_restore_stays_in_the_footer_and_posts_nothing(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-refused");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = to_confirmation(vcx, &shell, &seed, &calls);
    let count = revisions(vcx, &seed.slot, seed.id).len();
    write_more(vcx, &seed, 0xd4);

    press(vcx, "button-Restore entire Arrangement");
    wait(vcx, |cx| {
        view.read_with(cx, |view, cx| {
            view.stage
                .as_ref()
                .is_some_and(|stage| stage.refusal(cx).is_some())
        })
    });
    settle(vcx);

    let (refusal, confirming) = view.read_with(vcx, |view, cx| {
        let stage = view.stage.as_ref().expect("stage");
        (stage.refusal(cx), stage.is_confirming(cx))
    });
    assert_eq!(refusal.as_deref(), Some(STALE));
    assert!(!confirming, "back to stage one");
    assert!(has(vcx, "two-stage-refusal"));
    assert_eq!(depth(vcx, &shell), 2, "the comparison stays open");
    assert!(toasts(&shell, vcx).is_empty());
    assert!(calls.borrow().is_empty(), "the hook did not run");
    assert_eq!(revisions(vcx, &seed.slot, seed.id).len(), count + 1);
    assert!(
        has(vcx, "button-Restore this revision"),
        "Ask is usable again"
    );
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac3_a_refused_undo_posts_the_refusal_and_writes_nothing(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-undo-refused");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = to_confirmation(vcx, &shell, &seed, &calls);
    let count = revisions(vcx, &seed.slot, seed.id).len();
    press(vcx, "button-Restore entire Arrangement");
    wait_for_revisions(vcx, &seed, count + 1);
    write_more(vcx, &seed, 0xd5);

    press(vcx, "button-Undo");
    wait(vcx, |cx| {
        toasts(&shell, cx).iter().any(|toast| toast == STALE)
    });
    settle(vcx);
    assert_eq!(revisions(vcx, &seed.slot, seed.id).len(), count + 2);
    assert_eq!(calls.borrow().len(), 1, "no second hook run");
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac2_the_arrangement_comparison_opens_on_lyrics_and_a_song_on_overview(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-default-tab");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    assert!(has(vcx, "compare-current-line-0-0"), "Lyrics is showing");
    finish(vcx, &seed.slot, &seed.dir);
}

fn song_edit(cx: &mut TestAppContext, seed: &Seed, title: &str, kind: RevisionKind, by: u8) {
    let future = seed
        .slot
        .with(|storage| storage.latest_revision(RevisionEntity::Song(seed.song)))
        .expect("open");
    let parent = cx.foreground_executor().block_test(future).expect("latest");
    let content = SongContent {
        title: title.to_owned(),
        aliases: Vec::new(),
        original_attribution: Some("Fr. Manoling Francisco".to_owned()),
        provenance: None,
        rights_notes: None,
        liturgical_moments: Vec::new(),
        occasions: Vec::new(),
    };
    let song = seed.song;
    let future = seed
        .slot
        .with(|storage| {
            storage.commit_song_edit(
                song,
                content,
                kind,
                (kind == RevisionKind::Edit).then(|| session(by)),
                parent,
            )
        })
        .expect("open");
    cx.foreground_executor().block_test(future).expect("commit");
}

#[gpui::test]
fn ac3_an_archived_song_offers_no_restore_in_its_own_history(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-archived-song");
    song_edit(cx, &seed, "Tinapay (imported)", RevisionKind::Import, 0x93);
    let song = seed.song;
    let task = seed
        .store
        .update(cx, |store, cx| store.archive_song(song, cx));
    assert_eq!(cx.foreground_executor().block_test(task), Ok(song));
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, HistoryTarget::Song(song), &calls);
    let rows = view.read_with(vcx, |view, _| view.sessions.clone());
    assert_eq!(
        rows[0].how, "Import",
        "a Song import is not an Arrangement replacement"
    );
    press(vcx, "history-session-1");
    compare(vcx, 1, 0);

    assert!(has(vcx, "compare-body"));
    assert!(!has(vcx, "button-Restore this revision"));
    assert!(has(vcx, "button-Back to History"));
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac3_an_archived_arrangement_offers_no_restore_this_revision(cx: &mut TestAppContext) {
    let seed = seeded(cx, "t29-archived-arrangement");
    let id = seed.id;
    let task = seed
        .store
        .update(cx, |store, cx| store.archive_arrangement(id, cx));
    assert_eq!(cx.foreground_executor().block_test(task), Ok(id));
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let _view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    press(vcx, "history-session-2");
    compare(vcx, 2, 0);
    assert!(has(vcx, "compare-body"));
    assert!(!has(vcx, "button-Restore this revision"));
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac1_the_list_works_from_the_keyboard(cx: &mut TestAppContext) {
    use super::list::Item;
    let seed = seeded(cx, "t29-keys");
    let (shell, vcx) = open_store(cx, seed.store.clone());
    let calls = Calls::default();
    let view = open_view(vcx, &shell, &seed.store, arrangement_target(&seed), &calls);
    let active = |vcx: &mut VisualTestContext| view.read_with(vcx, |view, _| view.active);
    let list_focused = |vcx: &mut VisualTestContext| {
        let focus = view.read_with(vcx, |view, _| view.focus.clone());
        vcx.update(|window, _| focus.is_focused(window))
    };

    // Enter on Current opens nothing.
    vcx.simulate_keystrokes("down");
    assert_eq!(active(vcx), Item::Revision(0, 0));
    press_key(vcx, "enter");
    assert_eq!(depth(vcx, &shell), 1);

    // Down to the third session, Right opens it, Down to its revision, Enter
    // opens the comparison.
    vcx.simulate_keystrokes("down down right down");
    assert_eq!(active(vcx), Item::Revision(2, 0));
    press_key(vcx, "enter");
    settle(vcx);
    assert_eq!(depth(vcx, &shell), 2);
    press(vcx, "button-Back to History");
    settle(vcx);
    assert!(list_focused(vcx));

    // Left on a revision steps to the session it belongs to.
    vcx.simulate_keystrokes("left");
    assert_eq!(active(vcx), Item::Session(2));

    // Tab leaves the list for the surface's close control, and the surface
    // keeps the key: Shift+Tab from the list wraps within it too.
    vcx.simulate_keystrokes("tab");
    assert!(!list_focused(vcx), "Tab moves past the list");
    assert_eq!(depth(vcx, &shell), 1);
    vcx.simulate_keystrokes("shift-tab");
    assert!(list_focused(vcx));
    vcx.simulate_keystrokes("shift-tab");
    assert!(!list_focused(vcx), "Shift+Tab wraps to the last control");
    vcx.simulate_keystrokes("tab");
    assert!(list_focused(vcx), "and Tab wraps back to the list");
    assert_eq!(depth(vcx, &shell), 1);
    finish(vcx, &seed.slot, &seed.dir);
}

#[gpui::test]
fn ac1_compact_header_buttons_name_themselves_on_hover(cx: &mut TestAppContext) {
    let (_shell, _editor, _id, vcx) = in_memory(cx, false);
    vcx.simulate_resize(gpui::size(gpui::px(800.0), gpui::px(600.0)));
    settle(vcx);
    for name in ["button-Arrangement details", "button-History"] {
        let bounds = vcx.debug_bounds(name).expect("the icon button");
        assert!(
            vcx.debug_bounds("control-tooltip").is_none(),
            "{name} before hover"
        );
        vcx.simulate_mouse_move(bounds.center(), None, gpui::Modifiers::default());
        vcx.executor()
            .advance_clock(ui::primitives::button::TOOLTIP_SHOW_DELAY);
        vcx.run_until_parked();
        assert!(
            vcx.debug_bounds("control-tooltip").is_some(),
            "{name} on hover"
        );
        vcx.simulate_mouse_move(
            gpui::point(gpui::px(2.0), gpui::px(2.0)),
            None,
            gpui::Modifiers::default(),
        );
        vcx.run_until_parked();
    }
}
