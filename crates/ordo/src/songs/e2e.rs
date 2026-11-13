//! The T2.4 Library path end to end: a temporary parish seeded through the
//! real storage, the real [`LibraryStore`] and the real shell over it. Text,
//! three Arrangement-scoped filters, a selection and a scroll survive an
//! Arrangement round trip, from every empty state as well.

use domain::{
    identity::{ArrangementId, ChoirId, SongId},
    song_library::{Evidence, SongLibrary},
};
use gpui::{AppContext as _, TestAppContext, VisualTestContext, point, px};
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
    assert!(present(vcx, "focused-editor") && present(vcx, "arrangement-overview"));
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
    assert!(present(vcx, "arrangement-overview"));
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
    assert!(present(vcx, "arrangement-overview"));
    click(vcx, "button-Back to Songs");
    vcx.run_until_parked();
    assert_eq!(
        summary(vcx),
        before,
        "query, selection, scroll offset, evidence and counts"
    );
    assert!(present(vcx, "songs-detail"));
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
