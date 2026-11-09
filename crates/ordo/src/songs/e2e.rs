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
            title: title.to_owned(),
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
fn t2_4_e2e_empty_library_keeps_its_state_across_a_choirs_round_trip(cx: &mut TestAppContext) {
    let store = cx.new(|cx| LibraryStore::with_library(SongLibrary::new(), cx));
    let (_shell, vcx) = open_store(cx, store);
    assert!(present(vcx, "songs-empty"));
    click(vcx, "tab-3");
    assert!(present(vcx, "choirs-empty") && !present(vcx, "songs-empty"));
    click(vcx, "tab-0");
    assert!(present(vcx, "songs-empty") && present(vcx, "songs-detail-steps"));
}

#[gpui::test]
fn t2_4_e2e_no_match_keeps_its_query_across_a_repertoire_round_trip(cx: &mut TestAppContext) {
    let store = cx.new(|cx| LibraryStore::with_library(memory().library, cx));
    let (shell, vcx) = open_store(cx, store);
    let songs = shell.read_with(vcx, |shell, _| shell.songs().clone());
    click(vcx, "songs-search");
    vcx.simulate_input("zzzz");
    vcx.run_until_parked();
    assert!(present(vcx, "songs-no-match"));
    let query = songs.read_with(vcx, |ws, _| ws.query().clone());

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
fn t2_4_e2e_archived_only_keeps_include_archived_and_selection_across_a_round_trip(
    cx: &mut TestAppContext,
) {
    let store = cx.new(|cx| LibraryStore::with_library(archived_only(), cx));
    let (shell, vcx) = open_store(cx, store);
    let songs = shell.read_with(vcx, |shell, _| shell.songs().clone());
    assert!(present(vcx, "songs-archived-only"));
    click(vcx, "button-Include archived");
    click(vcx, "songs-row-0");
    let (query, selected) = songs.read_with(vcx, |ws, _| (ws.query().clone(), ws.selected()));
    assert!(query.include_archived && selected.is_some());
    click(vcx, "arrangement-open-0");
    assert!(present(vcx, "arrangement-overview"));
    click(vcx, "button-Back to Songs");
    songs.read_with(vcx, |ws, _| {
        assert_eq!(ws.query(), &query);
        assert_eq!(ws.selected(), selected);
    });
    assert!(present(vcx, "songs-detail"));
}
