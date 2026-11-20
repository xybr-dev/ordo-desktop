//! Behavior of the Songs workspace through the shell that hosts it, over a
//! `LibraryStore::with_library` store (no storage) and one test over a real
//! store in a temporary directory.

use std::time::Duration;

use domain::{
    identity::{ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId, SongId, Timestamp},
    song_library::{
        ArrangementDraft, ChoirDraft, OccurrenceDraft, SectionDraft, SongDraft, SongLibrary,
    },
};
use gpui::{
    AppContext as _, Entity, Focusable, Modifiers, TestAppContext, VisualTestContext, point, px,
    size,
};

use super::{SongsWorkspace, list};
use crate::{
    BUILD_IDENTITY,
    shell::{ShellView, rail::Destination},
    songs::{LibraryState, LibraryStore, StorageSlot},
};

fn entity(n: u16) -> EntityId {
    let [low, high] = n.to_le_bytes();
    let mut bytes = [high; 16];
    bytes[0] = low;
    EntityId::from_random_bytes(bytes)
}

fn song_id(n: u16) -> SongId {
    SongId::from_entity(entity(n))
}

fn at() -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000)
}

/// One Arrangement with two sections and a repeating Sequence. Identities use
/// `base` and the five bytes after it.
fn add_arrangement(
    library: &mut SongLibrary,
    base: u16,
    song: SongId,
    title: &str,
    language: &str,
    choir: ChoirId,
    lyrics: &str,
) -> ArrangementId {
    let id = ArrangementId::from_entity(entity(base));
    let verse = SectionId::from_entity(entity(base + 1));
    let chorus = SectionId::from_entity(entity(base + 2));
    let occurrence = |n: u16, section| {
        OccurrenceDraft::new(OccurrenceId::from_entity(entity(base + n)), section)
    };
    library
        .create_arrangement(
            id,
            song,
            at(),
            ArrangementDraft {
                title: Some(title.to_owned()),
                languages: vec![language.to_owned()],
                rendition_attribution: None,
                notes: None,
                occasions: Vec::new(),
                sections: vec![
                    SectionDraft::labeled(verse, "Verse 1", lyrics),
                    SectionDraft::labeled(chorus, "Chorus", "Refrain line"),
                ],
                sequence: vec![
                    occurrence(3, verse),
                    occurrence(4, chorus),
                    occurrence(5, verse),
                ],
            },
            vec![choir],
        )
        .expect("arrangement");
    id
}

fn choirs(library: &mut SongLibrary) -> (ChoirId, ChoirId) {
    let a = ChoirId::from_entity(entity(1));
    let b = ChoirId::from_entity(entity(2));
    library
        .create_choir(a, at(), ChoirDraft::named("Koro San Roque"))
        .expect("choir");
    library
        .create_choir(b, at(), ChoirDraft::named("Youth Choir"))
        .expect("choir");
    (a, b)
}

/// Panis Angelicus (alias, two Arrangements), Ave Maria, and an archived Song.
fn seeded() -> SongLibrary {
    let mut library = SongLibrary::new();
    let (a, b) = choirs(&mut library);
    library
        .create_song(
            song_id(10),
            at(),
            SongDraft {
                title: "Panis Angelicus".to_owned(),
                aliases: vec!["Bread of Angels".to_owned()],
                original_attribution: Some("Thomas Aquinas".to_owned()),
                ..SongDraft::default()
            },
        )
        .expect("song");
    add_arrangement(
        &mut library,
        20,
        song_id(10),
        "Latin SATB",
        "Latin",
        a,
        "Panis angelicus fit panis",
    );
    add_arrangement(
        &mut library,
        30,
        song_id(10),
        "Cebuano",
        "Cebuano",
        b,
        "Tinapay sa langit",
    );
    library
        .create_song(song_id(11), at(), SongDraft::titled("Ave Maria"))
        .expect("song");
    add_arrangement(
        &mut library,
        40,
        song_id(11),
        "Ave Maria SATB",
        "Latin",
        a,
        "Ave Maria gratia plena",
    );
    library
        .create_song(song_id(12), at(), SongDraft::titled("Old Hymn"))
        .expect("song");
    add_arrangement(
        &mut library,
        50,
        song_id(12),
        "Old Hymn SATB",
        "English",
        a,
        "Of old we sang",
    );
    library.archive_song(song_id(12), at()).expect("archive");
    library
}

/// A shell on Library → Songs over `library`.
fn open(
    cx: &mut TestAppContext,
    library: SongLibrary,
) -> (Entity<ShellView>, &mut VisualTestContext) {
    let store = cx.new(|cx| LibraryStore::with_library(library, cx));
    open_store(cx, store)
}

fn open_store(
    cx: &mut TestAppContext,
    store: Entity<LibraryStore>,
) -> (Entity<ShellView>, &mut VisualTestContext) {
    cx.update(crate::init_ui).expect("ui");
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let mut shell = ShellView::new(BUILD_IDENTITY, window, cx);
        shell.set_library_store(store, window, cx);
        shell.open(Destination::Library, cx);
        window.activate_window();
        let focus = shell.focus_handle(cx);
        window.focus(&focus, cx);
        shell
    });
    cx.simulate_resize(size(px(1920.0), px(1080.0)));
    cx.run_until_parked();
    (shell, cx)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be on screen"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

fn present(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

fn workspace(shell: &Entity<ShellView>, cx: &mut VisualTestContext) -> Entity<SongsWorkspace> {
    shell.read_with(cx, |shell, _| shell.songs().clone())
}

/// Sets the search text and recomputes, as typing does.
fn search(workspace: &Entity<SongsWorkspace>, cx: &mut VisualTestContext, text: &'static str) {
    cx.update(|window, cx| {
        workspace.update(cx, |ws, cx| {
            ws.search_field().set_value(text, window, cx);
            ws.refresh(cx);
        });
    });
    cx.run_until_parked();
}

#[gpui::test]
fn ac1_library_shows_four_views_and_songs_are_real(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx, seeded());
    for tab in ["tab-0", "tab-1", "tab-2", "tab-3"] {
        assert!(present(cx, tab), "{tab}");
    }
    assert!(!present(cx, "tab-4"));
    // Active Songs only: two rows, by title, and no fixture placeholder.
    assert!(present(cx, "songs-row-0") && present(cx, "songs-row-1"));
    assert!(!present(cx, "songs-row-2"));
    assert!(present(cx, "row-Ave Maria") && present(cx, "row-Panis Angelicus"));
    assert!(!present(cx, "row-Songs \u{b7} entry 1"));
    // The other two local views keep today's fixture rows, not the workspace.
    for tab in ["tab-1", "tab-2"] {
        click(cx, tab);
        assert!(!present(cx, "songs-list-pane"), "{tab}");
        assert!(present(cx, "shell-primary-list"), "{tab}");
    }
    shell.read_with(cx, |shell, _| assert!(shell.editing().is_none()));
}

#[gpui::test]
fn ac2_ac3_empty_library_keeps_two_panes_and_creates_through_the_new_song_panel(
    cx: &mut TestAppContext,
) {
    let (_shell, cx) = open(cx, SongLibrary::new());
    assert!(present(cx, "songs-empty"));
    assert!(present(cx, "songs-create") && present(cx, "songs-import"));
    assert!(present(cx, "songs-new"));
    assert!(present(cx, "songs-detail-steps"));
    assert!(!present(cx, "songs-search"));
    // Import stays unavailable and does nothing when clicked.
    click(cx, "songs-import");
    assert!(!present(cx, "new-song-form"));
    // Create song and New song both open the New song panel.
    click(cx, "songs-create");
    assert!(present(cx, "new-song-form"));
    click(cx, "button-Cancel");
    cx.run_until_parked();
    click(cx, "songs-new");
    assert!(present(cx, "new-song-form"));
    assert!(present(cx, "songs-empty"));
}

#[gpui::test]
fn ac2_ac3_populated_selected_and_no_match_states(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx, seeded());
    assert!(present(cx, "songs-search") && present(cx, "songs-filter-choir"));
    assert!(present(cx, "songs-filter-language") && present(cx, "songs-filter-occasion"));
    assert!(present(cx, "songs-include-archived"));
    assert!(present(cx, "songs-detail-none"));
    // Selected: the detail pane shows the Song and its Arrangements.
    click(cx, "songs-row-1");
    assert!(present(cx, "songs-detail"));
    assert!(present(cx, "arrangement-open-0") && present(cx, "arrangement-open-1"));
    // No match: names the text and offers the one correction.
    let workspace = workspace(&shell, cx);
    search(&workspace, cx, "zzzz");
    assert!(present(cx, "songs-no-match") && present(cx, "button-Clear search and filters"));
    click(cx, "button-Clear search and filters");
    assert!(!present(cx, "songs-no-match") && present(cx, "songs-row-0"));
    workspace.read_with(cx, |ws, cx| {
        assert_eq!(ws.search_field().value(cx).as_ref(), "");
    });
}

#[gpui::test]
fn ac2_ac3_archived_only_state_offers_include_archived(cx: &mut TestAppContext) {
    let mut library = SongLibrary::new();
    let (a, _) = choirs(&mut library);
    library
        .create_song(song_id(10), at(), SongDraft::titled("Only Hymn"))
        .expect("song");
    add_arrangement(
        &mut library,
        20,
        song_id(10),
        "Only SATB",
        "English",
        a,
        "Words",
    );
    library.archive_song(song_id(10), at()).expect("archive");
    let (_shell, cx) = open(cx, library);
    assert!(present(cx, "songs-archived-only"));
    assert!(!present(cx, "songs-no-match") && !present(cx, "songs-empty"));
    click(cx, "button-Include archived");
    assert!(!present(cx, "songs-archived-only") && present(cx, "songs-row-0"));
}

#[gpui::test]
fn ac2_ac3_unavailable_store_names_the_failure(cx: &mut TestAppContext) {
    let store = cx.new(|cx| LibraryStore::unavailable("disk is full", cx));
    let (_shell, cx) = open_store(cx, store);
    assert!(present(cx, "songs-unavailable"));
    assert!(!present(cx, "songs-search"));
}

#[gpui::test]
fn ac2_ac3_loading_store_shows_loading_and_a_neutral_detail(cx: &mut TestAppContext) {
    let store = cx.new(LibraryStore::loading);
    let (_shell, cx) = open_store(cx, store);
    assert!(present(cx, "songs-loading") && present(cx, "songs-detail-waiting"));
    assert!(!present(cx, "songs-search") && !present(cx, "songs-unavailable"));
}

#[gpui::test]
fn ac2_ac3_a_file_where_the_parish_folder_belongs_is_unavailable(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let file = std::env::temp_dir().join(format!("ordo-songs-blocked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&file);
    std::fs::write(&file, b"not a folder").expect("blocking file");
    let slot = StorageSlot::new();
    let store = cx.new(|cx| LibraryStore::open(file.clone(), slot.clone(), cx));
    for _ in 0..400 {
        cx.run_until_parked();
        if store.read_with(cx, |store, _| *store.state() != LibraryState::Loading) {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    store.read_with(cx, |store, _| match store.state() {
        LibraryState::Unavailable(message) => assert!(!message.is_empty()),
        other => panic!("expected Unavailable, got {other:?}"),
    });
    assert!(!slot.is_open());
    let (_shell, cx) = open_store(cx, store);
    assert!(present(cx, "songs-unavailable") && present(cx, "songs-detail-unavailable"));
    let _ = std::fs::remove_file(&file);
}

#[gpui::test]
fn ac6_evidence_lines_counts_and_search_never_writes(cx: &mut TestAppContext) {
    let library = seeded();
    let before = library.clone();
    let (shell, cx) = open(cx, library);
    let workspace = workspace(&shell, cx);
    search(&workspace, cx, "bread");
    let alias = workspace.read_with(cx, |ws, _| list::evidence_line(&ws.hits()[0]));
    assert_eq!(alias.as_deref(), Some("Alias \u{b7} Bread of Angels"));
    assert!(present(cx, "row-Panis Angelicus"));
    search(&workspace, cx, "tinapay");
    let lyric = workspace.read_with(cx, |ws, _| list::evidence_line(&ws.hits()[0]));
    assert_eq!(lyric.as_deref(), Some("Lyric \u{b7} Tinapay sa langit"));
    // A title hit has no evidence line.
    search(&workspace, cx, "ave maria");
    let title = workspace.read_with(cx, |ws, _| list::evidence_line(&ws.hits()[0]));
    assert_eq!(title, None);
    // A Choir filter: Panis Angelicus matches 1 of its 2 Arrangements.
    search(&workspace, cx, "");
    click(cx, "songs-filter-choir");
    click(cx, "songs-option-0");
    let counts = workspace.read_with(cx, |ws, _| {
        ws.hits()
            .iter()
            .map(|hit| (hit.song, list::count_line(hit, true)))
            .collect::<Vec<_>>()
    });
    assert!(counts.contains(&(song_id(10), Some("1 of 2 Arrangements".to_owned()))));
    assert!(counts.contains(&(song_id(11), Some("1 of 1 Arrangement".to_owned()))));
    assert!(present(cx, "songs-chip-0") && present(cx, "songs-chip-remove-0"));
    click(cx, "songs-chip-remove-0");
    assert!(!present(cx, "songs-chip-0"));
    let store = workspace.read_with(cx, |ws, _| ws.store().clone());
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
}

#[gpui::test]
fn ac7_arrangement_round_trip_restores_query_filters_selection_and_scroll(cx: &mut TestAppContext) {
    let mut library = SongLibrary::new();
    let (a, _) = choirs(&mut library);
    for n in 0..40u16 {
        let id = song_id(1000 + n);
        library
            .create_song(id, at(), SongDraft::titled(format!("Song {n:02}")))
            .expect("song");
        let base = 2000 + n * 8;
        add_arrangement(
            &mut library,
            base,
            id,
            &format!("Setting {n:02}"),
            "Latin",
            a,
            "Gloria in excelsis",
        );
    }
    let same = library.clone();
    let (shell, cx) = open(cx, library);
    // Type into the real field, set a Choir filter, select, then scroll.
    click(cx, "songs-search");
    cx.simulate_input("song");
    cx.run_until_parked();
    click(cx, "songs-filter-choir");
    click(cx, "songs-option-0");
    click(cx, "songs-filter-choir");
    click(cx, "songs-row-0");
    let workspace = workspace(&shell, cx);
    workspace.update(cx, |ws, _| ws.scroll_list_to(point(px(0.0), px(-240.0))));
    cx.run_until_parked();
    let (query, selected, offset) = workspace.read_with(cx, |ws, _| {
        (ws.query().clone(), ws.selected(), ws.list_offset())
    });
    assert_eq!(query.text, "song");
    assert_eq!(query.choirs.len(), 1);
    assert!(selected.is_some());
    assert_eq!(offset.y, px(-240.0));
    // Where the selected row is on screen, which is what the operator sees;
    // repaint first so the scrolled position is the one measured.
    workspace.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let row_before = cx
        .debug_bounds("songs-row-0")
        .expect("selected row painted");

    click(cx, "arrangement-open-0");
    assert!(present(cx, "focused-editor") && present(cx, "arrangement-overview"));
    assert!(present(cx, "arrangement-section-0") && present(cx, "arrangement-section-1"));
    assert!(present(cx, "arrangement-sequence-2"));
    assert!(!present(cx, "songs-search"));
    shell.read_with(cx, |shell, _| {
        assert!(shell.editing_arrangement().is_some())
    });

    // The store announcing a change mid-trip must not lose the state.
    let store = workspace.read_with(cx, |ws, _| ws.store().clone());
    store.update(cx, |store, cx| store.replace_library(same, cx));
    cx.run_until_parked();

    click(cx, "button-Back to Songs");
    assert!(!present(cx, "focused-editor") && present(cx, "songs-search"));
    cx.run_until_parked();
    workspace.read_with(cx, |ws, _| {
        assert_eq!(ws.query(), &query);
        assert_eq!(ws.selected(), selected);
        assert_eq!(ws.list_offset(), offset);
    });
    let row_after = cx
        .debug_bounds("songs-row-0")
        .expect("selected row painted");
    assert_eq!(row_after.origin.y, row_before.origin.y);
    shell.read_with(cx, |shell, _| {
        assert!(shell.editing_arrangement().is_none())
    });
}

#[gpui::test]
fn ac9_include_archived_is_off_by_default_and_shows_a_badge(cx: &mut TestAppContext) {
    let library = seeded();
    let before = library.clone();
    let (shell, cx) = open(cx, library);
    let workspace = workspace(&shell, cx);
    assert!(!present(cx, "row-Old Hymn"));
    workspace.read_with(cx, |ws, _| assert!(!ws.query().include_archived));
    click(cx, "songs-include-archived");
    assert!(present(cx, "row-Old Hymn") && present(cx, "songs-row-2"));
    // Selecting the archived Song says it is inspect-only.
    click(cx, "row-Old Hymn");
    assert!(present(cx, "songs-archived-notice"));
    let store = workspace.read_with(cx, |ws, _| ws.store().clone());
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
}

#[gpui::test]
fn storage_backed_store_lists_seeded_songs(cx: &mut TestAppContext) {
    use storage::{NewSong, Storage, StorageConfig};

    cx.executor().allow_parking();
    let dir = std::env::temp_dir().join(format!("ordo-songs-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let seed = Storage::spawn(StorageConfig::in_directory(&dir));
    for title in ["Seeded One", "Seeded Two"] {
        cx.foreground_executor()
            .block_test(seed.create_song(NewSong {
                title: title.to_owned(),
                aliases: Vec::new(),
                original_attribution: None,
                provenance: None,
                rights_notes: None,
                liturgical_moments: Vec::new(),
                occasions: Vec::new(),
            }))
            .expect("seed song");
    }
    cx.foreground_executor().block_test(seed.shutdown());

    let slot = StorageSlot::new();
    let store = cx.new(|cx| LibraryStore::open(dir.clone(), slot.clone(), cx));
    for _ in 0..400 {
        cx.run_until_parked();
        if store.read_with(cx, |store, _| *store.state() != LibraryState::Loading) {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    store.read_with(cx, |store, _| {
        assert_eq!(store.state(), &LibraryState::Ready)
    });
    let (_shell, cx) = open_store(cx, store);
    assert!(present(cx, "row-Seeded One") && present(cx, "row-Seeded Two"));
    // The app-level slot owns the handle; stopping it is what quit does.
    assert!(slot.is_open());
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
    assert!(!slot.is_open());
    let _ = std::fs::remove_dir_all(&dir);
}
