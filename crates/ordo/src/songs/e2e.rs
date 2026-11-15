//! The T2.4 Library path end to end: a temporary parish seeded through the
//! real storage, the real [`LibraryStore`] and the real shell over it. Text,
//! three Arrangement-scoped filters, a selection and a scroll survive an
//! Arrangement round trip, from every empty state as well.

use domain::{
    identity::{ArrangementId, ChoirId, SongId},
    song_library::{Evidence, SongLibrary},
};
use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext, point, px};
use storage::{NewArrangement, NewChoir, NewSection, NewSong, Storage, StorageConfig};

use super::{
    LibraryStore,
    choirs::tests::{at, click, entity, memory, open_parish, open_store, present, temp_dir},
};

const LINE: &str = "Sa tanang adlaw dayandayan ang gugma";

async fn song(storage: &Storage, title: &str) -> SongId {
    storage
        .create_song(NewSong {
            title: title.to_owned(),
            aliases: Vec::new(),
            original_attribution: None,
            provenance: None,
            rights_notes: None,
            liturgical_moments: Vec::new(),
            occasions: Vec::new(),
        })
        .await
        .expect("song")
        .id
}

async fn arrangement(
    storage: &Storage,
    song: SongId,
    title: &str,
    language: &str,
    choir: ChoirId,
    occasion: &str,
) -> ArrangementId {
    storage
        .create_arrangement(NewArrangement {
            song,
            title: Some(title.to_owned()),
            notes: None,
            languages: vec![language.to_owned()],
            choirs: vec![choir],
            rendition_attribution: None,
            occasions: vec![occasion.to_owned()],
            sections: vec![
                NewSection {
                    label: Some("Verse 1".to_owned()),
                    lyrics: LINE.to_owned(),
                },
                NewSection {
                    label: Some("Chorus".to_owned()),
                    lyrics: "Refrain line".to_owned(),
                },
            ],
            sequence: vec![0, 1, 0],
        })
        .await
        .expect("arrangement")
        .id
}

/// Thirty Cebuano Christmas Songs for the Koro, Tinapay sa Langit with an
/// English Youth Choir Arrangement and an archived one, Ave Maria for the
/// Youth Choir at Lent, and an archived Song. All of them sing `LINE`.
async fn seed(storage: &Storage) {
    let named = |name: &str| NewChoir {
        name: name.to_owned(),
        short_label: None,
        languages: Vec::new(),
        notes: None,
    };
    let koro = storage
        .create_choir(named("Koro Sto. Niño"))
        .await
        .expect("choir")
        .id;
    let youth = storage
        .create_choir(named("Youth Choir"))
        .await
        .expect("choir")
        .id;
    storage
        .create_choir(named("Old Choir"))
        .await
        .expect("choir");
    for n in 0..30 {
        let id = song(storage, &format!("Kanta {n:02}")).await;
        let title = format!("Kanta {n:02} SATB");
        arrangement(storage, id, &title, "Cebuano", koro, "Christmas").await;
    }
    let tinapay = song(storage, "Tinapay sa Langit").await;
    arrangement(
        storage,
        tinapay,
        "Cebuano SATB",
        "Cebuano",
        koro,
        "Christmas",
    )
    .await;
    arrangement(storage, tinapay, "English SATB", "English", youth, "Lent").await;
    let retired = arrangement(
        storage,
        tinapay,
        "Old setting",
        "Cebuano",
        koro,
        "Christmas",
    )
    .await;
    storage.archive_arrangement(retired).await.expect("archive");
    let ave = song(storage, "Ave Maria").await;
    arrangement(storage, ave, "Ave Maria SATB", "Cebuano", youth, "Lent").await;
    let old = song(storage, "Old Hymn").await;
    arrangement(storage, old, "Old Hymn SATB", "Cebuano", koro, "Christmas").await;
    storage.archive_song(old).await.expect("archive");
}

#[gpui::test]
fn t2_4_e2e_query_filters_selection_and_scroll_survive_an_arrangement_round_trip(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let dir = temp_dir("songs-e2e");
    let storage = Storage::spawn(StorageConfig::in_directory(&dir));
    cx.foreground_executor().block_test(seed(&storage));
    cx.foreground_executor().block_test(storage.shutdown());
    let (slot, store) = open_parish(cx, &dir);
    let (shell, vcx) = open_store(cx, store);
    let songs = shell.read_with(vcx, |shell, _| shell.songs().clone());

    // The lyric line finds every active Arrangement that sings it.
    click(vcx, "songs-search");
    vcx.simulate_input("dayandayan");
    vcx.run_until_parked();
    songs.read_with(vcx, |ws, _| {
        assert_eq!(ws.hits().len(), 32, "30 Kanta + Tinapay + Ave Maria");
        for hit in ws.hits() {
            assert!(
                matches!(
                    hit.evidence.first(),
                    Some(Evidence::Lyric { line, .. }) if line.contains("dayandayan")
                ),
                "every hit names the lyric line"
            );
        }
    });

    // Three Arrangement-scoped filters: Choir, language and occasion.
    for filter in [
        "songs-filter-choir",
        "songs-filter-language",
        "songs-filter-occasion",
    ] {
        click(vcx, filter);
        click(vcx, "songs-option-0");
        click(vcx, filter);
    }
    let summary = |cx: &mut VisualTestContext| {
        songs.read_with(cx, |ws, _| {
            let hits: Vec<_> = ws
                .hits()
                .iter()
                .map(|hit| {
                    (
                        hit.song,
                        hit.evidence.clone(),
                        hit.matching_arrangements.len(),
                        hit.total_arrangements,
                    )
                })
                .collect();
            (ws.query().clone(), hits)
        })
    };
    let (query, hits) = summary(vcx);
    assert_eq!(query.text, "dayandayan");
    assert_eq!(
        (
            query.choirs.len(),
            query.languages.len(),
            query.occasions.len()
        ),
        (1, 1, 1)
    );
    assert_eq!(
        hits.len(),
        31,
        "Ave Maria is the Youth Choir's, so it drops out"
    );
    // Tinapay sa Langit matches through one of its two active Arrangements.
    let counts = |of: (usize, usize)| {
        hits.iter()
            .filter(|(_, _, matching, total)| (*matching, *total) == of)
            .count()
    };
    assert_eq!(counts((1, 2)), 1);
    assert_eq!(counts((1, 1)), 30);

    // Select a Song, scroll, then Open an Arrangement and come Back.
    click(vcx, "songs-row-0");
    songs.update(vcx, |ws, _| ws.scroll_list_to(point(px(0.0), px(-240.0))));
    vcx.run_until_parked();
    songs.update(vcx, |_, cx| cx.notify());
    vcx.run_until_parked();
    let selected = songs.read_with(vcx, |ws, _| ws.selected());
    let offset = songs.read_with(vcx, |ws, _| ws.list_offset());
    assert!(selected.is_some() && offset.y == px(-240.0));
    let row_before = vcx
        .debug_bounds("songs-row-0")
        .expect("selected row painted");

    click(vcx, "arrangement-open-0");
    assert!(present(vcx, "focused-editor") && present(vcx, "arrangement-lyrics"));
    click(vcx, "button-Back to Songs");
    vcx.run_until_parked();

    let (query_after, hits_after) = summary(vcx);
    assert_eq!(query_after, query);
    assert_eq!(
        hits_after, hits,
        "the same hits, evidence and m-of-n counts"
    );
    songs.read_with(vcx, |ws, _| {
        assert_eq!(ws.selected(), selected);
        assert_eq!(ws.list_offset(), offset);
    });
    let row_after = vcx
        .debug_bounds("songs-row-0")
        .expect("selected row painted");
    assert_eq!(
        row_after.origin.y, row_before.origin.y,
        "the same place on screen"
    );

    let outcome = vcx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
    let _ = std::fs::remove_dir_all(&dir);
}

fn archived_only() -> SongLibrary {
    let mut library = memory().library;
    for n in [10, 11] {
        library
            .archive_song(SongId::from_entity(entity(n)), at())
            .expect("archive");
    }
    library
}

#[gpui::test]
fn t2_4_e2e_empty_library_keeps_its_query_across_a_choirs_round_trip(cx: &mut TestAppContext) {
    let store = cx.new(|cx| LibraryStore::with_library(SongLibrary::new(), cx));
    let (shell, vcx) = open_store(cx, store);
    let songs = shell.read_with(vcx, |shell, _| shell.songs().clone());
    assert!(present(vcx, "songs-empty"));
    // The empty state draws no search box, but the query is the workspace's:
    // text and Include archived set now are still there after the round trip.
    let field = songs.read_with(vcx, |ws, _| ws.search_field().clone());
    vcx.update(|window, cx| field.set_value("dayandayan", window, cx));
    songs.update(vcx, |ws, cx| ws.set_include_archived(true, cx));
    vcx.run_until_parked();
    let query = songs.read_with(vcx, |ws, _| ws.query().clone());
    assert_eq!(query.text, "dayandayan");
    assert!(query.include_archived);

    click(vcx, "tab-3");
    assert!(present(vcx, "choirs-empty") && !present(vcx, "songs-empty"));
    click(vcx, "tab-0");
    assert!(present(vcx, "songs-empty") && present(vcx, "songs-detail-steps"));
    songs.read_with(vcx, |ws, cx| {
        assert_eq!(ws.query(), &query);
        assert_eq!(ws.search_field().value(cx).as_ref(), "dayandayan");
    });
}

#[gpui::test]
fn t2_4_e2e_no_match_keeps_its_query_and_filter_across_a_repertoire_round_trip(
    cx: &mut TestAppContext,
) {
    let store = cx.new(|cx| LibraryStore::with_library(memory().library, cx));
    let (shell, vcx) = open_store(cx, store);
    let songs = shell.read_with(vcx, |shell, _| shell.songs().clone());
    click(vcx, "songs-filter-choir");
    click(vcx, "songs-option-0");
    click(vcx, "songs-filter-choir");
    click(vcx, "songs-search");
    vcx.simulate_input("zzzz");
    vcx.run_until_parked();
    assert!(present(vcx, "songs-no-match"));
    let query = songs.read_with(vcx, |ws, _| ws.query().clone());
    assert_eq!(query.text, "zzzz");
    assert_eq!(
        query.choirs.len(),
        1,
        "the Choir filter is part of the query"
    );

    // Choirs, its repertoire, an Arrangement, and Back to Choirs.
    click(vcx, "tab-3");
    click(vcx, "choirs-row-0");
    click(vcx, "arrangement-open-0");
    assert!(present(vcx, "arrangement-lyrics"));
    click(vcx, "button-Back to Choirs");
    assert!(present(vcx, "choirs-detail"));
    click(vcx, "tab-0");
    assert!(present(vcx, "songs-no-match"));
    songs.read_with(vcx, |ws, _| assert_eq!(ws.query(), &query));
}

#[gpui::test]
fn t2_4_e2e_archived_only_keeps_evidence_scroll_and_selection_across_a_round_trip(
    cx: &mut TestAppContext,
) {
    let store = cx.new(|cx| LibraryStore::with_library(archived_only(), cx));
    let (shell, vcx) = open_store(cx, store);
    let songs = shell.read_with(vcx, |shell, _| shell.songs().clone());
    assert!(present(vcx, "songs-archived-only"));
    click(vcx, "button-Include archived");
    click(vcx, "songs-row-0");
    // Two rows never overflow the list, so the offset stays at the top; the
    // overflowing scroll case is the main E2E above.
    let summary = |cx: &mut VisualTestContext| {
        songs.read_with(cx, |ws, _| {
            let hits: Vec<_> = ws
                .hits()
                .iter()
                .map(|hit| {
                    (
                        hit.song,
                        hit.evidence.clone(),
                        hit.matching_arrangements.len(),
                        hit.total_arrangements,
                    )
                })
                .collect();
            (ws.query().clone(), ws.selected(), ws.list_offset(), hits)
        })
    };
    let before = summary(vcx);
    assert!(before.0.include_archived && before.1.is_some() && !before.3.is_empty());
    click(vcx, "arrangement-open-0");
    assert!(present(vcx, "arrangement-lyrics"));
    click(vcx, "button-Back to Songs");
    vcx.run_until_parked();
    assert_eq!(
        summary(vcx),
        before,
        "query, selection, scroll offset, evidence and counts"
    );
    assert!(present(vcx, "songs-detail"));
}

/// Wheels the Arrangement details body to its end, as an operator would, so
/// Notes is clear of the two-row footer before it is clicked.
fn scroll_details_to_end(vcx: &mut VisualTestContext) {
    let card = vcx
        .debug_bounds("overlay-Arrangement details")
        .expect("the panel is open");
    let at = card.center();
    vcx.simulate_mouse_move(at, None, gpui::Modifiers::default());
    vcx.simulate_event(gpui::ScrollWheelEvent {
        position: at,
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), -card.size.height)),
        modifiers: gpui::Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    vcx.run_until_parked();
}

/// T2.5: a near-duplicate title is inspected against two candidates and backed
/// out of twice with the form untouched; then Create commits one Song with no
/// Arrangements, and a reopen from disk finds it and leaves the candidates as
/// they were.
#[gpui::test]
fn t2_5_e2e_near_duplicate_inspect_two_candidates_then_create_and_reload(cx: &mut TestAppContext) {
    use super::new_song::tests::{
        click as press, form_state, open_panel, seed_near_matches, settle_depth, songs_of, stop,
        type_into,
    };

    cx.executor().allow_parking();
    let dir = temp_dir("t25-e2e");
    let seeding = Storage::spawn(StorageConfig::in_directory(&dir));
    let ids = cx
        .foreground_executor()
        .block_test(seed_near_matches(&seeding));
    cx.foreground_executor().block_test(seeding.shutdown());

    let (slot, store) = open_parish(cx, &dir);
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    let before = store.read_with(vcx, |store, _| store.library().clone());

    let panel = open_panel(&shell, vcx);
    vcx.simulate_input("Tinapay sa  Lang\u{ed}t");
    type_into(vcx, "new-song-composer", "Fr. Manoling Francisco");
    type_into(vcx, "new-song-aliases", "Bread from Heaven, Tinapay");
    type_into(vcx, "new-song-moment", "Communion");

    assert!(present(vcx, "button-3 possible existing Songs"));
    press(vcx, "button-3 possible existing Songs");
    let typed = form_state(&panel, vcx);
    assert_eq!(typed.2, [ids.exact, ids.near, ids.archived]);

    for (at, expected) in [(0, ids.exact), (1, ids.near)] {
        let selector = ["new-song-candidate-0", "new-song-candidate-1"][at];
        press(vcx, selector);
        settle_depth(&songs, vcx, 2);
        panel.read_with(vcx, |panel, _| {
            assert_eq!(panel.previewed(), Some(expected))
        });
        assert!(present(vcx, "new-song-preview-arrangements"));
        press(vcx, "button-Back");
        settle_depth(&songs, vcx, 1);
        assert_eq!(
            form_state(&panel, vcx),
            typed,
            "Back leaves the form as typed"
        );
    }
    store.read_with(vcx, |store, _| assert_eq!(store.library(), &before));
    songs.read_with(vcx, |songs, _| assert_eq!(songs.selected(), None));

    press(vcx, "button-Create song");
    crate::songs::choirs::tests::wait(vcx, |cx| {
        songs.read_with(cx, |songs, cx| songs.overlay_depth(cx)) == 0
    });
    settle_depth(&songs, vcx, 0);
    let created = songs.read_with(vcx, |songs, _| songs.selected().expect("selected"));
    assert!(![ids.exact, ids.near, ids.archived, ids.unrelated].contains(&created));
    assert!(present(vcx, "songs-no-arrangements"));
    store.read_with(vcx, |store, _| {
        assert_eq!(store.library().songs().count(), before.songs().count() + 1);
    });
    stop(vcx, &slot);

    // Reopen from disk.
    let (slot, store) = open_parish(cx, &dir);
    let (id, work) = store.read_with(cx, |store, _| {
        let work = store
            .library()
            .song(created)
            .expect("the created Song persisted");
        (created, work.clone())
    });
    assert_eq!(work.title(), "Tinapay sa  Lang\u{ed}t");
    assert_eq!(work.original_attribution(), Some("Fr. Manoling Francisco"));
    assert_eq!(work.aliases(), ["Bread from Heaven", "Tinapay"]);
    let moments: Vec<_> = work.liturgical_moments().iter().cloned().collect();
    assert_eq!(moments, ["Communion"]);
    store.read_with(cx, |store, _| {
        let library = store.library();
        assert!(library.arrangements_for_song(id).unwrap().is_empty());
        assert_eq!(library.songs().count(), before.songs().count() + 1);
        for (song, count) in [
            (ids.exact, 2),
            (ids.near, 1),
            (ids.archived, 1),
            (ids.unrelated, 1),
        ] {
            assert_eq!(library.arrangements_for_song(song).unwrap().len(), count);
            assert_eq!(library.song(song).unwrap(), before.song(song).unwrap());
        }
    });
    cx.foreground_executor()
        .block_test(slot.shutdown())
        .expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

/// T2.6: a blank-title two-language Arrangement shared by two Choirs gets its
/// derived label; an edit goes through the shared notice, a second edit is
/// cancelled, and a duplicate is independent. A reopen from disk finds both
/// with their own identities and revision histories.
#[gpui::test]
fn t2_6_e2e_create_edit_cancel_duplicate_then_reload_with_independent_histories(
    cx: &mut TestAppContext,
) {
    use super::arrangement_details::tests::{
        arrangement_count, choir_named, label_of, open_edit, open_new, write,
    };
    use super::new_song::tests::{
        click as press, parish, settle, settle_depth, songs_of, stop, type_into,
    };
    use domain::{
        revisions::{RevisionEntity, RevisionKind},
        song_library::ChoirDraft,
    };

    let (dir, slot, store, seeded) = parish(cx, "t26-e2e");
    let (shell, vcx) = open_store(cx, store.clone());
    let koro = choir_named(&store, "Koro Sto. Ni\u{f1}o", vcx).expect("koro");
    let youth = write(&store, vcx, |store, cx| {
        store.create_choir(ChoirDraft::named("Youth Choir"), cx)
    });
    let before = arrangement_count(&store, vcx);

    // Create: two languages, two Choirs, no title.
    open_new(&shell, seeded.near, vcx);
    press(vcx, "button-Cebuano");
    press(vcx, "button-English");
    press(vcx, "button-Koro Sto. Ni\u{f1}o");
    press(vcx, "button-Youth Choir");
    press(vcx, "button-Create Arrangement");
    crate::songs::choirs::tests::wait(vcx, |vcx| arrangement_count(&store, vcx) == before + 1);
    settle(vcx);
    let original = shell.read_with(vcx, |shell, _| {
        shell.editing_arrangement().expect("focused")
    });
    let label = label_of(&store, original, vcx);
    assert!(
        label.contains("Cebuano") && label.contains("English"),
        "{label}"
    );
    store.read_with(vcx, |store, _| {
        let arrangement = store.library().arrangement(original).unwrap();
        assert_eq!(arrangement.title(), None);
        assert_eq!(arrangement.choirs().len(), 2);
        assert!(arrangement.choirs().contains(&koro) && arrangement.choirs().contains(&youth));
    });

    // Edit through the shared notice.
    open_edit(&shell, original, vcx);
    assert!(present(vcx, "ad-shared-notice"));
    scroll_details_to_end(vcx);
    type_into(vcx, "ad-notes", "Revised once");
    press(vcx, "button-Save details");
    crate::songs::choirs::tests::wait(vcx, |vcx| {
        store.read_with(vcx, |store, _| {
            store.library().arrangement(original).unwrap().notes() == Some("Revised once")
        })
    });
    settle(vcx);

    // A cancelled edit stores nothing.
    open_edit(&shell, original, vcx);
    scroll_details_to_end(vcx);
    type_into(vcx, "ad-notes", " and discarded");
    press(vcx, "button-Cancel");
    settle_depth(&songs_of(&shell, vcx), vcx, 0);
    store.read_with(vcx, |store, _| {
        assert_eq!(
            store.library().arrangement(original).unwrap().notes(),
            Some("Revised once")
        );
    });

    // Duplicate.
    open_edit(&shell, original, vcx);
    press(vcx, "button-Duplicate Arrangement");
    crate::songs::choirs::tests::wait(vcx, |vcx| arrangement_count(&store, vcx) == before + 2);
    settle(vcx);
    let copy = shell.read_with(vcx, |shell, _| {
        shell.editing_arrangement().expect("focused")
    });
    assert_ne!(copy, original);
    stop(vcx, &slot);

    // Reopen from disk.
    let (slot, store) = open_parish(cx, &dir);
    let history = |cx: &mut TestAppContext, id: ArrangementId| {
        let future = slot
            .with(|storage| storage.revisions(RevisionEntity::Arrangement(id)))
            .expect("storage is open");
        cx.foreground_executor()
            .block_test(future)
            .expect("revisions")
    };
    store.read_with(cx, |store, _| {
        let library = store.library();
        let source = library.arrangement(original).expect("source persisted");
        let duplicate = library.arrangement(copy).expect("copy persisted");
        assert_eq!(source.notes(), Some("Revised once"));
        assert_eq!(duplicate.languages(), source.languages());
        assert_eq!(duplicate.choirs(), source.choirs());
        assert_eq!(library.arrangements().count(), before + 2);
    });
    let source_history = history(cx, original);
    let copy_history = history(cx, copy);
    assert_eq!(source_history.len(), 2, "creation and one edit");
    assert_eq!(source_history[0].kind, RevisionKind::Creation);
    assert_eq!(copy_history.len(), 1, "the copy starts its own chain");
    for revision in &copy_history {
        assert!(source_history.iter().all(|other| other.id != revision.id));
    }
    cx.foreground_executor()
        .block_test(slot.shutdown())
        .expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The golden file of a T2.7 fixture, which is the frame plus its (empty)
/// warnings, so the thumbnail's serialization must be a prefix of it.
fn golden(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden-frames")
        .join(format!("{name}.txt"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Presses and releases `key`, as a focused button needs to activate.
fn tap(vcx: &mut VisualTestContext, key: &str) {
    let keystroke = gpui::Keystroke::parse(key).expect("key");
    vcx.simulate_event(gpui::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    vcx.simulate_event(gpui::KeyUpEvent { keystroke });
}

/// Waits for a structural edit to be written, without moving the clock (a
/// toast would time out).
fn written(editor: &Entity<super::lyrics::ArrangementEditor>, vcx: &mut VisualTestContext) {
    use super::new_song::tests::settle;
    crate::songs::choirs::tests::wait(vcx, |vcx| {
        editor.read_with(vcx, |editor, cx| {
            editor.save_state(cx) == Some(crate::edit_session::SaveState::Saved)
        })
    });
    settle(vcx);
}

/// Types `lines` into the focused field, pressing Enter between them.
fn type_lines(vcx: &mut VisualTestContext, lines: &[&str]) {
    for (at, line) in lines.iter().enumerate() {
        if at > 0 {
            vcx.simulate_keystrokes("enter");
        }
        vcx.simulate_input(line);
    }
}

/// T2.7: Verse 1 and Chorus are created by keyboard, the lyrics are split and
/// joined by text and by button, the Chorus is moved with Alt+Up, removed
/// through the confirmation and brought back by the toast's Undo and by
/// Ctrl+Z. A reopen from disk shows the canonical thumbnails the goldens pin,
/// the section order, and the Sequence untouched with its occurrence ids.
#[gpui::test]
fn t2_7_e2e_keyboard_sections_split_join_reorder_remove_undo_then_reload(cx: &mut TestAppContext) {
    use super::lyrics::tests::{
        area_value, dialog, entity, open_editor, parish_blank, saved, sections_of,
        thumbnail_frames, toasts,
    };
    use super::new_song::tests::{click as press, settle, stop};
    use domain::{
        identity::{HistorySessionId, OccurrenceId, SectionId},
        song_library::{ArrangementLyrics, OccurrenceDraft},
    };

    const VERSE: [&str; 4] = [
        "Ginoó, among tabang sa buntag",
        "Niño sa among kasingkasing",
        "Paz ug kalipay sa tanan",
        "Vida nga walay kataposan",
    ];
    const CHORUS: [&str; 2] = ["Aleluya, mga anak sa adlaw", "Ginoó, among kalipay ug awit"];
    let two_groups = format!("{}\n{}\n\n{}\n{}", VERSE[0], VERSE[1], VERSE[2], VERSE[3]);
    let one_group = VERSE.join("\n");

    let (dir, slot, store, shell, id, vcx) = parish_blank(cx, "t27-e2e");
    let stored = |vcx: &mut VisualTestContext| -> ArrangementLyrics {
        store.read_with(vcx, |store, _| {
            store.library().arrangement(id).expect("stored").lyrics()
        })
    };
    let editor = open_editor(&shell, id, vcx);

    // Verse 1 by keyboard: Enter on the focused Add first section, the
    // prefilled name replaced, Enter again; the lyric editor has focus.
    assert!(present(vcx, "button-Add first section"));
    tap(vcx, "enter");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Add section"));
    vcx.simulate_keystrokes("ctrl-a");
    vcx.simulate_input("Verse 1");
    vcx.simulate_keystrokes("enter");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1"]);
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("area");
    assert!(vcx.update(|window, cx| area.focus_handle(cx).is_focused(window)));

    // One slide of four lines, split by text with a blank line, joined by
    // deleting it, split by its button, joined by its button.
    type_lines(vcx, &VERSE);
    settle(vcx);
    assert_eq!(area_value(&editor, vcx), one_group);
    vcx.simulate_keystrokes("up up end enter");
    settle(vcx);
    assert_eq!(area_value(&editor, vcx), two_groups, "split by text");
    vcx.simulate_keystrokes("backspace");
    settle(vcx);
    assert_eq!(area_value(&editor, vcx), one_group, "joined by text");
    press(vcx, "button-Split after line 2");
    saved(&editor, vcx);
    assert_eq!(area_value(&editor, vcx), two_groups, "split by button");
    assert_eq!(stored(vcx).sections[0].lyrics, two_groups);
    press(vcx, "thumb-1");
    settle(vcx);
    press(vcx, "button-Join slide 2 with slide 1");
    saved(&editor, vcx);
    assert_eq!(area_value(&editor, vcx), one_group, "joined by button");
    assert_eq!(stored(vcx).sections[0].lyrics, one_group);
    // The two slide groups that stay: split once more, by text.
    press(vcx, "thumb-0");
    settle(vcx);
    vcx.simulate_keystrokes("ctrl-home down end enter");
    settle(vcx);
    assert_eq!(area_value(&editor, vcx), two_groups);
    saved(&editor, vcx);
    assert_eq!(stored(vcx).sections[0].lyrics, two_groups);

    vcx.simulate_keystrokes("shift-f6");
    settle(vcx);
    tap(vcx, "enter");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Add section"));
    vcx.simulate_keystrokes("ctrl-a");
    vcx.simulate_input("Chorus");
    vcx.simulate_keystrokes("enter");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    type_lines(vcx, &CHORUS);
    saved(&editor, vcx);
    let chorus_text = CHORUS.join("\n");
    let before_seed = stored(vcx);
    assert_eq!(before_seed.sections[1].lyrics, chorus_text);
    let verse_id: SectionId = before_seed.sections[0].id;
    let chorus_id: SectionId = before_seed.sections[1].id;

    // The Sequence has no editor yet (T2.8): leave, seed one Verse 1 and one
    // Chorus occurrence through the store, and come back to a new visit.
    press(vcx, "button-Back to Songs");
    crate::songs::choirs::tests::wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.editing().is_none())
    });
    settle(vcx);
    let occurrence =
        |n: u8, section| OccurrenceDraft::new(OccurrenceId::from_entity(entity(n)), section);
    let (verse_use, chorus_use) = (entity(91), entity(90));
    let mut seeded_lyrics = stored(vcx);
    seeded_lyrics.sequence = vec![occurrence(91, verse_id), occurrence(90, chorus_id)];
    let states: Vec<_> = seeded_lyrics
        .sections
        .iter()
        .map(|section| section.pagination)
        .collect();
    super::arrangement_details::tests::write(&store, vcx, |store, cx| {
        store.save_arrangement_lyrics(
            id,
            seeded_lyrics,
            HistorySessionId::from_entity(entity(8)),
            cx,
        )
    });
    let sequence_ids = |vcx: &mut VisualTestContext| -> Vec<OccurrenceId> {
        stored(vcx).sequence.iter().map(|o| o.id).collect()
    };
    let expected_sequence = [
        OccurrenceId::from_entity(verse_use),
        OccurrenceId::from_entity(chorus_use),
    ];
    assert_eq!(sequence_ids(vcx), expected_sequence);
    let editor = open_editor(&shell, id, vcx);

    // Alt+Up on the Chorus row: authoring order only.
    press(vcx, "arrangement-section-1");
    settle(vcx);
    assert_eq!(
        super::lyrics::tests::focused_row(&editor, vcx),
        Some(chorus_id),
        "the Chorus row has focus"
    );
    vcx.simulate_keystrokes("alt-up");
    written(&editor, vcx);
    assert_eq!(sections_of(&editor, vcx), ["Chorus", "Verse 1"]);
    assert_eq!(
        sequence_ids(vcx),
        expected_sequence,
        "the Sequence did not move"
    );

    // Remove the Chorus through the confirmation; the toast's Undo restores it.
    let reordered = stored(vcx);
    press(vcx, "button-Remove section");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Remove Chorus?"));
    press(vcx, "button-Remove section");
    written(&editor, vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1"]);
    assert_eq!(
        sequence_ids(vcx),
        [expected_sequence[0]],
        "its occurrence went with it"
    );
    assert_eq!(toasts(&shell, vcx), ["Removed Chorus."]);
    press(vcx, "button-Undo");
    written(&editor, vcx);
    assert_eq!(
        stored(vcx),
        reordered,
        "Undo restored lyrics, positions and approval"
    );
    assert_eq!(
        stored(vcx)
            .sections
            .iter()
            .map(|section| section.pagination)
            .collect::<Vec<_>>(),
        states,
        "Undo restored every section's approval"
    );

    // And once more, undone with Ctrl+Z from the sections list.
    press(vcx, "arrangement-section-0");
    settle(vcx);
    press(vcx, "button-Remove section");
    settle(vcx);
    press(vcx, "button-Remove section");
    written(&editor, vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1"]);
    press(vcx, "arrangement-section-0");
    settle(vcx);
    assert!(super::lyrics::tests::focused_row(&editor, vcx).is_some());
    vcx.simulate_keystrokes("ctrl-z");
    written(&editor, vcx);
    assert_eq!(stored(vcx), reordered, "Ctrl+Z restored it as well");

    // Reopen the parish from disk.
    press(vcx, "button-Back to Songs");
    crate::songs::choirs::tests::wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.editing().is_none())
    });
    settle(vcx);
    stop(vcx, &slot);
    let (slot, store) = open_parish(cx, &dir);
    let (shell, vcx) = open_store(cx, store.clone());
    let reloaded = store.read_with(vcx, |store, _| {
        store.library().arrangement(id).expect("persisted").lyrics()
    });
    let names: Vec<_> = reloaded
        .sections
        .iter()
        .map(|section| (section.id, section.label.clone().unwrap_or_default()))
        .collect();
    assert_eq!(
        names,
        [
            (chorus_id, "Chorus".to_owned()),
            (verse_id, "Verse 1".to_owned())
        ]
    );
    let occurrences: Vec<_> = reloaded
        .sequence
        .iter()
        .map(|o| (o.id, o.section))
        .collect();
    assert_eq!(
        occurrences,
        [
            (OccurrenceId::from_entity(verse_use), verse_id),
            (OccurrenceId::from_entity(chorus_use), chorus_id),
        ],
        "Sequence order and occurrence ids are the seeded ones"
    );
    assert_eq!(
        reloaded
            .sections
            .iter()
            .map(|section| section.pagination)
            .collect::<Vec<_>>(),
        states,
        "each section's approval persisted"
    );
    let editor = open_editor(&shell, id, vcx);
    let pinned = |name: &str, frame: &str| {
        assert_eq!(
            golden(name),
            format!("{frame}warnings 0\n"),
            "{name}: the thumbnail is the golden frame"
        );
    };
    press(vcx, "arrangement-section-0");
    settle(vcx);
    let chorus_frames = thumbnail_frames(&editor, vcx);
    assert_eq!(chorus_frames.len(), 1);
    pinned("lyrics-t2-7-chorus-slide-1", &chorus_frames[0]);
    press(vcx, "arrangement-section-1");
    settle(vcx);
    let verse_frames = thumbnail_frames(&editor, vcx);
    assert_eq!(verse_frames.len(), 2);
    pinned("lyrics-t2-7-verse-1-slide-1", &verse_frames[0]);
    pinned("lyrics-t2-7-verse-1-slide-2", &verse_frames[1]);
    assert_eq!(area_value(&editor, vcx), two_groups);

    cx.foreground_executor()
        .block_test(slot.shutdown())
        .expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The element that owns keyboard focus, by what the E2E can name: a Sequence
/// row or slide tile, Add first occurrence, the lyric area, a control the test
/// has met (and named with [`learn`]), or else just the F6 region.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Owner {
    Row(domain::identity::OccurrenceId),
    Tile(domain::identity::OccurrenceId, usize),
    AddFirst,
    LyricArea,
    Named(&'static str),
    Region(String),
}

/// Names the control that has focus right now, so later steps can assert it.
fn learn(
    known: &mut Vec<(&'static str, gpui::FocusHandle)>,
    name: &'static str,
    vcx: &mut VisualTestContext,
) {
    let handle = vcx
        .update(|window, cx| window.focused(cx))
        .expect("a control has focus");
    assert!(
        known.iter().all(|(_, other)| *other != handle),
        "{name} is a control that already has a name"
    );
    known.push((name, handle));
}

fn owner(
    editor: &Entity<super::lyrics::ArrangementEditor>,
    shell: &Entity<crate::shell::ShellView>,
    known: &[(&'static str, gpui::FocusHandle)],
    vcx: &mut VisualTestContext,
) -> Owner {
    let region = super::lyrics::tests::region_now(shell, vcx);
    let (rows, kids, add_first, area) = editor.read_with(vcx, |editor, cx| {
        (
            editor.sequence.rows.clone(),
            editor.sequence.kids.clone(),
            editor.sequence.add_first.clone(),
            editor.lyric_area().map(|area| area.focus_handle(cx)),
        )
    });
    vcx.update(|window, _| {
        if let Some((id, _)) = rows.iter().find(|(_, handle)| handle.is_focused(window)) {
            Owner::Row(*id)
        } else if let Some(((id, slide), _)) =
            kids.iter().find(|(_, handle)| handle.is_focused(window))
        {
            Owner::Tile(*id, *slide)
        } else if add_first.is_focused(window) {
            Owner::AddFirst
        } else if area.as_ref().is_some_and(|handle| handle.is_focused(window)) {
            Owner::LyricArea
        } else if let Some((name, _)) = known.iter().find(|(_, handle)| handle.is_focused(window)) {
            Owner::Named(name)
        } else {
            Owner::Region(region)
        }
    })
}

/// A slide's golden file: the frame plus its (empty) warnings.
fn pinned(name: &str, frame: &str) {
    assert_eq!(
        golden(name),
        format!("{frame}warnings 0\n"),
        "{name}: the frame is the golden"
    );
}

/// T2.8: the Sequence is built, repeated, edited once, reordered, trimmed and
/// restored by keyboard on a real parish store. A reopen from disk shows the
/// flattened order and the canonical frames the goldens pin.
#[gpui::test]
fn t2_8_e2e_build_repeat_edit_once_reorder_delete_undo_then_reload(cx: &mut TestAppContext) {
    use super::lyrics::tests::{
        area_value, entity, open_editor, parish_blank, region_now, saved as lyrics_saved,
        sections_of, toasts,
    };
    use super::new_song::tests::{click as press, settle, stop};
    use crate::songs::{
        choirs::tests::wait,
        sequence::{SequenceSelection, occurrence_name, slide_keys},
    };
    use domain::{
        identity::{HistorySessionId, OccurrenceId, SectionId},
        song_library::{SectionDraft, flatten},
    };

    const VERSE_1: &str = "Ginoó, among tabang sa buntag\nNiño sa among kasingkasing\n\nPaz ug kalipay sa tanan\nVida nga walay kataposan";
    const CHORUS_FIRST: &str = "Aleluya, mga anak sa adlaw";
    const CHORUS_EDITED: [&str; 2] = ["Aleluya, awit sa tanan", "Ginoó, among kalipay ug awit"];
    const VERSE_2: &str = "Sa kagabhion nagpabilin ka\nGinoó, among kahayag\n\nSa kabuntagon nagbalik kami\nSalamat sa imong gugma";

    let (dir, slot, store, shell, id, vcx) = parish_blank(cx, "t28-e2e");
    let (verse_1, chorus, verse_2) = (
        SectionId::from_entity(entity(61)),
        SectionId::from_entity(entity(62)),
        SectionId::from_entity(entity(63)),
    );
    // Verse 1, Chorus and Verse 2 are written through the store; the Sequence
    // starts empty (a section never places itself).
    let mut seeded = store.read_with(vcx, |store, _| {
        store.library().arrangement(id).expect("stored").lyrics()
    });
    seeded.sections = vec![
        SectionDraft::labeled(verse_1, "Verse 1", VERSE_1),
        SectionDraft::labeled(chorus, "Chorus", CHORUS_FIRST),
        SectionDraft::labeled(verse_2, "Verse 2", VERSE_2),
    ];
    seeded.sequence.clear();
    super::arrangement_details::tests::write(&store, vcx, |store, cx| {
        store.save_arrangement_lyrics(id, seeded, HistorySessionId::from_entity(entity(8)), cx)
    });
    let stored = |vcx: &mut VisualTestContext| {
        store.read_with(vcx, |store, _| {
            store.library().arrangement(id).expect("stored").lyrics()
        })
    };
    let editor = open_editor(&shell, id, vcx);
    let mut known = Vec::new();
    let ids = |vcx: &mut VisualTestContext| -> Vec<OccurrenceId> {
        editor.read_with(vcx, |editor, cx| {
            let content = editor.content(cx).expect("content");
            content.sequence.iter().map(|occurrence| occurrence.id).collect()
        })
    };
    let names = |vcx: &mut VisualTestContext| -> Vec<String> {
        editor.read_with(vcx, |editor, cx| {
            let content = editor.content(cx).expect("content");
            content
                .sequence
                .iter()
                .map(|occurrence| occurrence_name(&content, occurrence.section))
                .collect()
        })
    };
    let selected = |vcx: &mut VisualTestContext| {
        editor.read_with(vcx, |editor, _| editor.sequence_selection)
    };
    let focus = |known: &[(&'static str, gpui::FocusHandle)], vcx: &mut VisualTestContext| {
        owner(&editor, &shell, known, vcx)
    };
    let on = |id: OccurrenceId| Some(SequenceSelection::occurrence(id));
    let region = |name: &str, vcx: &mut VisualTestContext| {
        assert_eq!(region_now(&shell, vcx), name);
    };

    // The Arrangement opens on Lyrics with focus in the header actions: Tab
    // past Back to the task tabs, Right to Sequence, Enter to open it.
    region("Workspace actions", vcx);
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    learn(&mut known, "Back to Songs", vcx);
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    assert_eq!(focus(&known, vcx), Owner::Region("Workspace actions".into()));
    learn(&mut known, "Lyrics tab", vcx);
    vcx.simulate_keystrokes("right");
    settle(vcx);
    learn(&mut known, "Sequence tab", vcx);
    vcx.simulate_keystrokes("enter");
    settle(vcx);
    assert_eq!(editor.read_with(vcx, |editor, cx| editor.tab(cx)), super::lyrics::TaskTab::Sequence);
    assert_eq!(
        focus(&known, vcx),
        Owner::AddFirst,
        "an empty Sequence focuses Add first occurrence"
    );
    assert!(ids(vcx).is_empty());
    assert_eq!(selected(vcx), None, "nothing is selected");

    // 01 Verse 1: Enter on Add first occurrence.
    tap(vcx, "enter");
    written(&editor, vcx);
    let first = ids(vcx);
    assert_eq!(names(vcx), ["Verse 1"]);
    assert_eq!(selected(vcx), on(first[0]));
    assert_eq!(focus(&known, vcx), Owner::Row(first[0]), "focus follows the new row");
    assert_eq!(toasts(&shell, vcx), ["Added Verse 1 at 01 in the Sequence."]);

    // 02 Chorus, 03 Verse 2, 04 Chorus: Add in Available sections, each placed
    // after the selection, focus staying on the pressed button.
    vcx.simulate_keystrokes("shift-f6");
    settle(vcx);
    region("Primary list", vcx);
    learn(&mut known, "Add Verse 1", vcx);
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    learn(&mut known, "Add Chorus", vcx);
    tap(vcx, "enter");
    written(&editor, vcx);
    assert_eq!(names(vcx), ["Verse 1", "Chorus"]);
    assert_eq!(selected(vcx), on(ids(vcx)[1]));
    assert_eq!(focus(&known, vcx), Owner::Named("Add Chorus"));
    assert_eq!(toasts(&shell, vcx), ["Added Chorus at 02 in the Sequence."]);
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    learn(&mut known, "Add Verse 2", vcx);
    tap(vcx, "enter");
    written(&editor, vcx);
    assert_eq!(names(vcx), ["Verse 1", "Chorus", "Verse 2"]);
    assert_eq!(selected(vcx), on(ids(vcx)[2]));
    assert_eq!(focus(&known, vcx), Owner::Named("Add Verse 2"));
    assert_eq!(toasts(&shell, vcx), ["Added Verse 2 at 03 in the Sequence."]);
    vcx.simulate_keystrokes("shift-tab");
    settle(vcx);
    assert_eq!(focus(&known, vcx), Owner::Named("Add Chorus"));
    tap(vcx, "enter");
    written(&editor, vcx);
    assert_eq!(names(vcx), ["Verse 1", "Chorus", "Verse 2", "Chorus"]);
    let [o1, o2, o3, o4]: [OccurrenceId; 4] = ids(vcx).try_into().expect("four occurrences");
    assert_eq!(first[0], o1);
    assert_eq!(selected(vcx), on(o4));
    assert_eq!(focus(&known, vcx), Owner::Named("Add Chorus"));
    assert_eq!(toasts(&shell, vcx), ["Added Chorus at 04 in the Sequence."]);
    assert_eq!(stored(vcx).sequence.len(), 4, "saved as it went");
    assert_eq!(
        stored(vcx).sequence.iter().map(|o| o.section).collect::<Vec<_>>(),
        [verse_1, chorus, verse_2, chorus],
        "04 is a second occurrence of the one Chorus section"
    );
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus", "Verse 2"]);

    // Edit the Chorus once, in Lyrics: F6 to the header actions (focus lands on
    // the active tab), Left, Enter; F6 to the sections, Tab to the Chorus row,
    // Enter to choose it; F6 and Tab to the lyric area; Ctrl+A and type.
    vcx.simulate_keystrokes("shift-f6");
    settle(vcx);
    region("Workspace actions", vcx);
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    assert_eq!(focus(&known, vcx), Owner::Named("Back to Songs"));
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    vcx.simulate_keystrokes("left");
    settle(vcx);
    vcx.simulate_keystrokes("enter");
    settle(vcx);
    assert_eq!(editor.read_with(vcx, |editor, cx| editor.tab(cx)), super::lyrics::TaskTab::Lyrics);
    vcx.simulate_keystrokes("f6");
    settle(vcx);
    region("Primary list", vcx);
    for _ in 0..3 {
        vcx.simulate_keystrokes("tab");
        settle(vcx);
    }
    assert_eq!(super::lyrics::tests::focused_row(&editor, vcx), Some(chorus));
    tap(vcx, "enter");
    settle(vcx);
    assert_eq!(editor.read_with(vcx, |editor, _| editor.selected_section()), Some(chorus));
    vcx.simulate_keystrokes("f6");
    settle(vcx);
    region("Main editor", vcx);
    for _ in 0..6 {
        if focus(&known, vcx) == Owner::LyricArea {
            break;
        }
        vcx.simulate_keystrokes("tab");
        settle(vcx);
    }
    assert_eq!(focus(&known, vcx), Owner::LyricArea);
    assert_eq!(area_value(&editor, vcx), CHORUS_FIRST);
    vcx.simulate_keystrokes("ctrl-a");
    type_lines(vcx, &CHORUS_EDITED);
    lyrics_saved(&editor, vcx);
    let edited = CHORUS_EDITED.join("\n");
    assert_eq!(stored(vcx).sections[1].lyrics, edited);
    assert_eq!(focus(&known, vcx), Owner::LyricArea);
    assert_eq!(ids(vcx), [o1, o2, o3, o4], "the edit touched no occurrence");

    // Back to Sequence: both Chorus occurrences share the edited frame.
    for _ in 0..4 {
        if region_now(&shell, vcx) == "Workspace actions" {
            break;
        }
        vcx.simulate_keystrokes("f6");
        settle(vcx);
    }
    region("Workspace actions", vcx);
    vcx.simulate_keystrokes("right");
    settle(vcx);
    vcx.simulate_keystrokes("enter");
    settle(vcx);
    assert_eq!(editor.read_with(vcx, |editor, cx| editor.tab(cx)), super::lyrics::TaskTab::Sequence);
    assert_eq!(selected(vcx), on(o4), "the selection survived the tab switch");
    vcx.simulate_keystrokes("f6");
    settle(vcx);
    vcx.simulate_keystrokes("f6");
    settle(vcx);
    region("Main editor", vcx);
    tap(vcx, "enter"); // Expand all, the first stop of the Sequence pane
    settle(vcx);
    assert_eq!(editor.read_with(vcx, |editor, _| editor.expanded.len()), 4);
    let chorus_key = slide_keys(&stored(vcx).sections[1].clone()).remove(0);
    assert_eq!(chorus_key, (Some("Chorus".to_owned()), edited.clone()));
    wait(vcx, |vcx| {
        settle(vcx);
        editor.update(vcx, |editor, _| editor.frame_of(&chorus_key).is_some())
    });
    assert!(crate::songs::choirs::tests::present(vcx, "sequence-child-1-0"));
    assert!(crate::songs::choirs::tests::present(vcx, "sequence-child-3-0"));
    let chorus_frame = editor
        .update(vcx, |editor, _| editor.frame_of(&chorus_key))
        .expect("composed");
    pinned("lyrics-t2-8-chorus-edited-slide-1", &chorus_frame.serialize());
    let content = editor.read_with(vcx, |editor, cx| editor.content(cx).expect("content"));
    for occurrence in [o2, o4] {
        let at = content.occurrence_index(occurrence).expect("occurrence");
        let section = content.sections.iter().find(|s| s.id == content.sequence[at].section).expect("section");
        assert_eq!(slide_keys(section).remove(0), chorus_key, "both occurrences use the one edited key");
    }

    // Alt+Down then Alt+Up on 01.
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    assert_eq!(focus(&known, vcx), Owner::Row(o1));
    assert_eq!(selected(vcx), on(o1), "selection follows focus");
    vcx.simulate_keystrokes("alt-down");
    written(&editor, vcx);
    assert_eq!(ids(vcx), [o2, o1, o3, o4]);
    assert_eq!(focus(&known, vcx), Owner::Row(o1));
    assert_eq!(selected(vcx), on(o1));
    assert_eq!(stored(vcx).sequence.iter().map(|o| o.id).collect::<Vec<_>>(), [o2, o1, o3, o4]);
    assert_eq!(toasts(&shell, vcx), ["Moved Verse 1 to 02."]);
    vcx.simulate_keystrokes("alt-up");
    written(&editor, vcx);
    assert_eq!(ids(vcx), [o1, o2, o3, o4]);
    assert_eq!(focus(&known, vcx), Owner::Row(o1));
    assert_eq!(stored(vcx).sequence.iter().map(|o| o.id).collect::<Vec<_>>(), [o1, o2, o3, o4]);
    assert_eq!(toasts(&shell, vcx), ["Moved Verse 1 to 01."]);

    // Focus 04 by Tab and press Delete: 03 is selected and focused.
    for _ in 0..30 {
        if focus(&known, vcx) == Owner::Row(o4) {
            break;
        }
        vcx.simulate_keystrokes("tab");
        settle(vcx);
    }
    assert_eq!(focus(&known, vcx), Owner::Row(o4));
    assert_eq!(selected(vcx), on(o4));
    vcx.simulate_keystrokes("delete");
    written(&editor, vcx);
    assert_eq!(ids(vcx), [o1, o2, o3]);
    assert_eq!(selected(vcx), on(o3), "03 is selected");
    assert_eq!(focus(&known, vcx), Owner::Row(o3), "and focused");
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus", "Verse 2"], "the section stays");
    assert_eq!(
        toasts(&shell, vcx),
        ["Removed 04 Chorus from the Sequence. The section stays in Lyrics."]
    );

    // Ctrl+Z puts 04 back, selected and focused.
    vcx.simulate_keystrokes("ctrl-z");
    wait(vcx, |vcx| {
        settle(vcx);
        ids(vcx) == [o1, o2, o3, o4]
    });
    written(&editor, vcx);
    assert_eq!(selected(vcx), on(o4), "04 is selected");
    assert_eq!(focus(&known, vcx), Owner::Row(o4), "and focused");
    assert_eq!(stored(vcx).sequence.iter().map(|o| o.id).collect::<Vec<_>>(), [o1, o2, o3, o4]);

    // Reopen the parish from disk.
    press(vcx, "button-Back to Songs");
    wait(vcx, |vcx| shell.read_with(vcx, |shell, _| shell.editing().is_none()));
    settle(vcx);
    stop(vcx, &slot);
    let (slot, store) = open_parish(cx, &dir);
    let (shell, vcx) = open_store(cx, store.clone());
    let reloaded = store.read_with(vcx, |store, _| {
        store.library().arrangement(id).expect("persisted").lyrics()
    });
    let order: Vec<_> = flatten(&reloaded)
        .iter()
        .map(|flat| (flat.occurrence, flat.position, flat.section, flat.slide, flat.slides_in_section))
        .collect();
    assert_eq!(
        order,
        [
            (o1, 0, verse_1, 0, 2),
            (o1, 0, verse_1, 1, 2),
            (o2, 1, chorus, 0, 1),
            (o3, 2, verse_2, 0, 2),
            (o3, 2, verse_2, 1, 2),
            (o4, 3, chorus, 0, 1),
        ],
        "exact flattened Cue order after reload"
    );
    let editor = open_editor(&shell, id, vcx);
    press(vcx, "tab-1");
    press(vcx, "button-Expand all");
    let frames_of = |section: usize| slide_keys(&reloaded.sections[section]);
    let wanted: Vec<_> = frames_of(0).into_iter().chain(frames_of(1)).chain(frames_of(2)).collect();
    wait(vcx, |vcx| {
        settle(vcx);
        editor.update(vcx, |editor, _| wanted.iter().all(|key| editor.frame_of(key).is_some()))
    });
    let frame = |section: usize, slide: usize, vcx: &mut VisualTestContext| {
        let key = frames_of(section).remove(slide);
        editor
            .update(vcx, |editor, _| editor.frame_of(&key))
            .expect("composed")
            .serialize()
    };
    for flat in flatten(&reloaded) {
        let at = reloaded.sections.iter().position(|s| s.id == flat.section).expect("section");
        let name = match (at, flat.slide) {
            (0, 0) => "lyrics-t2-7-verse-1-slide-1",
            (0, 1) => "lyrics-t2-7-verse-1-slide-2",
            (1, 0) => "lyrics-t2-8-chorus-edited-slide-1",
            (2, 0) => "lyrics-t2-8-verse-2-slide-1",
            (2, 1) => "lyrics-t2-8-verse-2-slide-2",
            other => panic!("unexpected flat slide {other:?}"),
        };
        pinned(name, &frame(at, flat.slide, vcx));
    }
    assert_eq!(area_value_after_reload(&reloaded), edited);

    cx.foreground_executor()
        .block_test(slot.shutdown())
        .expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

fn area_value_after_reload(content: &domain::song_library::ArrangementLyrics) -> String {
    content.sections[1].lyrics.clone()
}
