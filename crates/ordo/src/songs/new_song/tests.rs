//! Behavior of the New song panel through the shell that hosts it. Tests that
//! write run over a real store in a temporary directory; the failure test
//! reads a `LibraryStore::with_library` store, whose empty slot stands in for
//! a storage that cannot be reached.

use std::path::{Path, PathBuf};

use domain::{
    identity::SongId,
    song_library::{SongLibrary, SongWork},
};
use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext, point, px};
use storage::{NewArrangement, NewChoir, NewSection, NewSong, Storage, StorageConfig};

use super::{NewSongPanel, build_draft};
use crate::{
    shell::ShellView,
    songs::{
        LibraryStore, SongsWorkspace, StorageSlot,
        choirs::tests::{click as click_now, memory, open_parish, open_store, present, temp_dir},
    },
};

/// The Songs a test parish holds, by their roles.
pub(crate) struct Seeded {
    /// "Tinapay sa Langit", composer Fr. Manoling Francisco, two Arrangements.
    pub exact: SongId,
    /// "Tinapay, sa Langit": a near match by punctuation, same composer.
    pub near: SongId,
    /// "Tinapay-sa-langit", archived, no composer: a near match.
    pub archived: SongId,
    /// "Ave Maria": unrelated to the near matches.
    pub unrelated: SongId,
}

async fn song(storage: &Storage, title: &str, composer: Option<&str>, aliases: &[&str]) -> SongId {
    storage
        .create_song(NewSong {
            title: title.to_owned(),
            aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
            original_attribution: composer.map(str::to_owned),
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
    choirs: Vec<domain::identity::ChoirId>,
) {
    storage
        .create_arrangement(NewArrangement {
            song,
            title: title.to_owned(),
            languages: vec![language.to_owned()],
            choirs,
            rendition_attribution: None,
            occasions: Vec::new(),
            sections: vec![NewSection {
                label: Some("Verse 1".to_owned()),
                lyrics: "Sa tanang adlaw dayandayan ang gugma".to_owned(),
            }],
            sequence: vec![0],
        })
        .await
        .expect("arrangement");
}

/// Seeds three near matches of "Tinapay sa Langit" (one archived) and one
/// unrelated Song, each with Arrangements.
pub(crate) async fn seed_near_matches(storage: &Storage) -> Seeded {
    let koro = storage
        .create_choir(NewChoir {
            name: "Koro Sto. Ni\u{f1}o".to_owned(),
            short_label: None,
            languages: Vec::new(),
            notes: None,
        })
        .await
        .expect("choir")
        .id;
    let exact = song(
        storage,
        "Tinapay sa Langit",
        Some("Fr. Manoling Francisco"),
        &[],
    )
    .await;
    arrangement(storage, exact, "Cebuano SATB", "Cebuano", vec![koro]).await;
    arrangement(storage, exact, "English unison", "English", Vec::new()).await;
    let near = song(
        storage,
        "Tinapay, sa Langit",
        Some("fr manoling francisco"),
        &[],
    )
    .await;
    arrangement(storage, near, "Cebuano unison", "Cebuano", Vec::new()).await;
    let archived = song(storage, "Tinapay-sa-langit", None, &[]).await;
    arrangement(storage, archived, "Old setting", "Cebuano", vec![koro]).await;
    storage.archive_song(archived).await.expect("archive");
    let unrelated = song(storage, "Ave Maria", Some("Franz Schubert"), &[]).await;
    arrangement(storage, unrelated, "Latin SATB", "Latin", vec![koro]).await;
    Seeded {
        exact,
        near,
        archived,
        unrelated,
    }
}

/// A seeded temporary parish, opened for a test.
fn parish(
    cx: &mut TestAppContext,
    tag: &str,
) -> (PathBuf, StorageSlot, Entity<LibraryStore>, Seeded) {
    cx.executor().allow_parking();
    let dir = temp_dir(tag);
    let seed = Storage::spawn(StorageConfig::in_directory(&dir));
    let ids = cx
        .foreground_executor()
        .block_test(seed_near_matches(&seed));
    cx.foreground_executor().block_test(seed.shutdown());
    let (slot, store) = open_parish(cx, &dir);
    (dir, slot, store, ids)
}

/// Stops the storage worker cleanly and removes the folder.
pub(crate) fn finish(cx: &mut VisualTestContext, slot: &StorageSlot, dir: &Path) {
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
    let _ = std::fs::remove_dir_all(dir);
}

/// Stops the storage worker cleanly and leaves the folder for a reopen.
pub(crate) fn stop(cx: &mut VisualTestContext, slot: &StorageSlot) {
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
}

pub(crate) fn songs_of(
    shell: &Entity<ShellView>,
    cx: &mut VisualTestContext,
) -> Entity<SongsWorkspace> {
    shell.read_with(cx, |shell, _| shell.songs().clone())
}

fn panel_of(songs: &Entity<SongsWorkspace>, cx: &mut VisualTestContext) -> Entity<NewSongPanel> {
    songs.read_with(cx, |songs, _| {
        songs.new_song().expect("the panel is open").clone()
    })
}

/// Lets any overlay slide finish so its controls are where they will stay.
pub(crate) fn settle(cx: &mut VisualTestContext) {
    for _ in 0..10 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(60));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

pub(crate) fn click(cx: &mut VisualTestContext, selector: &'static str) {
    click_now(cx, selector);
    settle(cx);
}

/// Opens the panel from the list header and returns it.
pub(crate) fn open_panel(
    shell: &Entity<ShellView>,
    cx: &mut VisualTestContext,
) -> Entity<NewSongPanel> {
    click(cx, "songs-new");
    panel_of(&songs_of(shell, cx), cx)
}

/// Clicks into a field's wrapper so it takes focus, then types.
pub(crate) fn type_into(cx: &mut VisualTestContext, selector: &'static str, text: &str) {
    click(cx, selector);
    cx.simulate_input(text);
    cx.run_until_parked();
}

/// What the form holds, for an exact before/after comparison.
pub(crate) fn form_state(
    panel: &Entity<NewSongPanel>,
    cx: &mut VisualTestContext,
) -> (Vec<String>, bool, Vec<SongId>) {
    panel.read_with(cx, |panel, cx| {
        (
            vec![
                panel.title_field().value(cx).to_string(),
                panel.composer_field().value(cx).to_string(),
                panel.aliases_field().value(cx).to_string(),
                panel.moments_field().value(cx).to_string(),
            ],
            panel.candidates_expanded(cx),
            panel.candidates().iter().map(|c| c.song).collect(),
        )
    })
}

pub(crate) fn titles(store: &Entity<LibraryStore>, cx: &mut VisualTestContext) -> Vec<String> {
    store.read_with(cx, |store, _| {
        store
            .library()
            .songs()
            .map(|song| song.title().to_owned())
            .collect()
    })
}

pub(crate) fn song_count(store: &Entity<LibraryStore>, cx: &mut VisualTestContext) -> usize {
    store.read_with(cx, |store, _| store.library().songs().count())
}

/// Reads a parish back from disk through a fresh store.
pub(crate) fn reopened(cx: &mut TestAppContext, dir: &Path) -> (StorageSlot, Entity<LibraryStore>) {
    open_parish(cx, dir)
}

pub(crate) fn find_title(
    store: &Entity<LibraryStore>,
    title: &str,
    cx: &mut TestAppContext,
) -> Option<(SongId, SongWork)> {
    store.read_with(cx, |store, _| {
        store
            .library()
            .songs()
            .find(|song| song.title() == title)
            .map(|song| (song.id(), song.clone()))
    })
}

pub(crate) fn depth(songs: &Entity<SongsWorkspace>, cx: &mut VisualTestContext) -> usize {
    songs.read_with(cx, |songs, cx| songs.overlay_depth(cx))
}

/// Waits until the layer holds exactly `depth` surfaces that are not leaving.
pub(crate) fn settle_depth(
    songs: &Entity<SongsWorkspace>,
    cx: &mut VisualTestContext,
    expected: usize,
) {
    for _ in 0..400 {
        settle(cx);
        if depth(songs, cx) == expected {
            return;
        }
    }
    panic!("the layer never reached depth {expected}");
}

#[test]
fn the_draft_splits_trims_and_drops_empties() {
    let draft = build_draft(
        "  Kyrie  ",
        "  ",
        " Lord have mercy, , Señor ten piedad ,",
        "Entrance,Communion , ",
    );
    assert_eq!(draft.title, "  Kyrie  ");
    assert_eq!(draft.original_attribution, None);
    assert_eq!(draft.aliases, ["Lord have mercy", "Señor ten piedad"]);
    assert_eq!(draft.liturgical_moments, ["Entrance", "Communion"]);
    let draft = build_draft("T", " Fr. M ", "", "");
    assert_eq!(draft.original_attribution.as_deref(), Some(" Fr. M "));
    assert!(draft.aliases.is_empty() && draft.liturgical_moments.is_empty());
}

#[gpui::test]
fn ac1_the_form_has_exactly_four_identity_fields_and_none_of_the_arrangement_ones(
    cx: &mut TestAppContext,
) {
    let store = cx.new(|cx| LibraryStore::with_library(memory().library, cx));
    let (shell, vcx) = open_store(cx, store);
    open_panel(&shell, vcx);
    for field in [
        "new-song-title",
        "new-song-composer",
        "new-song-aliases",
        "new-song-moment",
    ] {
        assert!(present(vcx, field), "{field}");
    }
    for absent in [
        "new-song-lyrics",
        "new-song-language",
        "new-song-choir",
        "new-song-section",
        "new-song-sections",
        "new-song-sequence",
        "new-song-boundary",
        "button-Add section",
        "button-Add Choir",
    ] {
        assert!(!present(vcx, absent), "{absent}");
    }
    // Primary and secondary actions, and nothing that creates an Arrangement.
    assert!(present(vcx, "button-Create song") && present(vcx, "button-Cancel"));
    assert!(!present(vcx, "button-Add first Arrangement"));
}

#[gpui::test]
fn ac1_create_is_unavailable_for_a_blank_or_whitespace_title(cx: &mut TestAppContext) {
    let (dir, slot, store, _) = parish(cx, "t25-blank");
    let (shell, vcx) = open_store(cx, store.clone());
    let panel = open_panel(&shell, vcx);
    let before = song_count(&store, vcx);
    panel.read_with(vcx, |panel, cx| assert!(!panel.can_create(cx)));
    click(vcx, "button-Create song");
    assert_eq!(song_count(&store, vcx), before);
    type_into(vcx, "new-song-title", "   ");
    panel.read_with(vcx, |panel, cx| assert!(!panel.can_create(cx)));
    click(vcx, "button-Create song");
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    assert_eq!(song_count(&store, vcx), before);
    assert!(panel.read_with(vcx, |panel, _| panel.error().is_none()));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac1_a_title_only_create_persists(cx: &mut TestAppContext) {
    let (dir, slot, store, _) = parish(cx, "t25-title-only");
    let (shell, vcx) = open_store(cx, store.clone());
    open_panel(&shell, vcx);
    vcx.simulate_input("Ginoo, Kaloy-i Kami");
    vcx.run_until_parked();
    click(vcx, "button-Create song");
    crate::songs::choirs::tests::wait(vcx, |cx| {
        titles(&store, cx).contains(&"Ginoo, Kaloy-i Kami".to_owned())
    });
    stop(vcx, &slot);
    let (slot, store) = reopened(cx, &dir);
    let (id, work) = find_title(&store, "Ginoo, Kaloy-i Kami", cx).expect("persisted");
    assert!(work.aliases().is_empty() && work.original_attribution().is_none());
    store.read_with(cx, |store, _| {
        assert!(
            store
                .library()
                .arrangements_for_song(id)
                .unwrap()
                .is_empty()
        );
    });
    cx.foreground_executor()
        .block_test(slot.shutdown())
        .expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn ac2_composer_aliases_and_moments_land_on_the_song_work_after_reopen(cx: &mut TestAppContext) {
    let (dir, slot, store, _) = parish(cx, "t25-identity");
    let (shell, vcx) = open_store(cx, store.clone());
    open_panel(&shell, vcx);
    vcx.simulate_input("Kordero sa Dios");
    type_into(vcx, "new-song-composer", "Fr. Carlo Magno");
    type_into(vcx, "new-song-aliases", " Lamb of God , Agnus Dei,, ");
    type_into(vcx, "new-song-moment", "Communion, Fraction Rite");
    click(vcx, "button-Create song");
    crate::songs::choirs::tests::wait(vcx, |cx| {
        titles(&store, cx).contains(&"Kordero sa Dios".to_owned())
    });
    stop(vcx, &slot);
    let (slot, store) = reopened(cx, &dir);
    let (id, work) = find_title(&store, "Kordero sa Dios", cx).expect("persisted");
    assert_eq!(work.original_attribution(), Some("Fr. Carlo Magno"));
    assert_eq!(work.aliases(), ["Lamb of God", "Agnus Dei"]);
    let moments: Vec<_> = work.liturgical_moments().iter().cloned().collect();
    assert_eq!(moments, ["Communion", "Fraction Rite"]);
    store.read_with(cx, |store, _| {
        // Nothing landed on any Arrangement: the new Song has none.
        assert!(
            store
                .library()
                .arrangements_for_song(id)
                .unwrap()
                .is_empty()
        );
    });
    cx.foreground_executor()
        .block_test(slot.shutdown())
        .expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn ac4_the_count_is_singular_plural_or_hidden(cx: &mut TestAppContext) {
    let (dir, slot, store, _) = parish(cx, "t25-count");
    let (shell, vcx) = open_store(cx, store);
    let panel = open_panel(&shell, vcx);
    // Nothing typed: no block at all.
    assert!(!present(vcx, "new-song-possible"));
    // One exact match.
    vcx.simulate_input("Ave Maria");
    vcx.run_until_parked();
    assert!(present(vcx, "button-1 possible existing Song"));
    assert!(!present(vcx, "button-1 possible existing Songs"));
    // Three near matches.
    panel.update(vcx, |panel, cx| {
        let window_free = panel.title_field().clone();
        let _ = window_free;
        cx.notify();
    });
    click(vcx, "new-song-title");
    vcx.simulate_keystrokes("cmd-a");
    vcx.simulate_keystrokes("ctrl-a");
    for _ in 0..9 {
        vcx.simulate_keystrokes("backspace");
    }
    vcx.simulate_input("Tinapay sa Langit");
    vcx.run_until_parked();
    assert!(present(vcx, "button-3 possible existing Songs"));
    // Nothing alike: hidden again.
    for _ in 0..17 {
        vcx.simulate_keystrokes("backspace");
    }
    vcx.simulate_input("Zzz");
    vcx.run_until_parked();
    assert!(!present(vcx, "new-song-possible"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac4_the_count_is_collapsed_then_lists_candidates_in_domain_order(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = parish(cx, "t25-order");
    let (shell, vcx) = open_store(cx, store);
    let panel = open_panel(&shell, vcx);
    vcx.simulate_input("Tinapay sa Langit");
    type_into(vcx, "new-song-composer", "Fr. Manoling Francisco");
    // Collapsed by default, and nothing is selected for the operator.
    assert!(present(vcx, "button-3 possible existing Songs"));
    assert!(!present(vcx, "new-song-candidates"));
    panel.read_with(vcx, |panel, cx| assert!(!panel.candidates_expanded(cx)));
    let songs = songs_of(&shell, vcx);
    songs.read_with(vcx, |songs, _| assert_eq!(songs.selected(), None));
    // Expanded: exact title first, then the near match with the same composer,
    // then the archived one.
    click(vcx, "button-3 possible existing Songs");
    assert!(present(vcx, "new-song-candidates"));
    panel.read_with(vcx, |panel, cx| {
        assert!(panel.candidates_expanded(cx));
        let order: Vec<_> = panel.candidates().iter().map(|c| c.song).collect();
        assert_eq!(order, [ids.exact, ids.near, ids.archived]);
        assert!(!order.contains(&ids.unrelated));
    });
    for at in [
        "new-song-candidate-0",
        "new-song-candidate-1",
        "new-song-candidate-2",
    ] {
        assert!(present(vcx, at), "{at}");
    }
    assert!(!present(vcx, "new-song-candidate-3"));
    assert!(present(vcx, "row-Tinapay sa Langit"));
    songs.read_with(vcx, |songs, _| assert_eq!(songs.selected(), None));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac4_choosing_a_candidate_opens_the_preview_and_not_the_song_page(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = parish(cx, "t25-preview");
    let (shell, vcx) = open_store(cx, store);
    let panel = open_panel(&shell, vcx);
    let songs = songs_of(&shell, vcx);
    vcx.simulate_input("Tinapay sa Langit");
    vcx.run_until_parked();
    click(vcx, "button-3 possible existing Songs");
    assert_eq!(depth(&songs, vcx), 1);
    click(vcx, "new-song-candidate-0");
    assert_eq!(depth(&songs, vcx), 2);
    assert_eq!(
        songs.read_with(vcx, |songs, cx| songs.overlay_top(cx)),
        Some("Tinapay sa Langit".into())
    );
    panel.read_with(vcx, |panel, _| {
        assert_eq!(panel.previewed(), Some(ids.exact))
    });
    // The workspace did not navigate: nothing is selected and no Song page.
    songs.read_with(vcx, |songs, _| assert_eq!(songs.selected(), None));
    assert!(present(vcx, "songs-detail-none"));
    assert!(!present(vcx, "songs-detail"));
    // Identity rows and the Arrangement summary, and nothing editable.
    for part in [
        "new-song-preview",
        "new-song-preview-composer",
        "new-song-preview-aliases",
        "new-song-preview-moments",
        "new-song-preview-arrangements",
        "button-Back",
        "button-Use this existing Song",
    ] {
        assert!(present(vcx, part), "{part}");
    }
    assert!(!present(vcx, "new-song-preview-lyrics"));
    assert!(!present(vcx, "button-Edit"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_back_and_escape_restore_the_form_exactly(cx: &mut TestAppContext) {
    let (dir, slot, store, _) = parish(cx, "t25-back");
    let (shell, vcx) = open_store(cx, store.clone());
    let panel = open_panel(&shell, vcx);
    let songs = songs_of(&shell, vcx);
    vcx.simulate_input("Tinapay sa Langit");
    type_into(vcx, "new-song-composer", "Fr. Manoling Francisco");
    type_into(vcx, "new-song-aliases", "Bread from Heaven");
    type_into(vcx, "new-song-moment", "Communion");
    click(vcx, "button-3 possible existing Songs");
    let before = form_state(&panel, vcx);
    let library_before = store.read_with(vcx, |store, _| store.library().clone());

    // Back.
    click(vcx, "new-song-candidate-1");
    assert_eq!(depth(&songs, vcx), 2);
    click(vcx, "button-Back");
    settle_depth(&songs, vcx, 1);
    assert_eq!(form_state(&panel, vcx), before);
    let title_focused = vcx.update(|window, cx| {
        panel
            .read(cx)
            .title_field()
            .focus_handle(cx)
            .is_focused(window)
    });
    // Focus goes back to the invoker, the candidate row, not to the title.
    assert!(!title_focused);
    assert!(present(vcx, "new-song-candidates"));

    // Escape.
    click(vcx, "new-song-candidate-2");
    assert_eq!(depth(&songs, vcx), 2);
    vcx.simulate_keystrokes("escape");
    settle_depth(&songs, vcx, 1);
    assert_eq!(form_state(&panel, vcx), before);
    assert!(songs.read_with(vcx, |songs, _| songs.new_song().is_some()));
    assert_eq!(
        songs.read_with(vcx, |songs, cx| songs.overlay_top(cx)),
        Some("New song".into())
    );
    panel.read_with(vcx, |panel, _| assert_eq!(panel.previewed(), None));
    store.read_with(vcx, |store, _| assert_eq!(store.library(), &library_before));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_use_existing_closes_everything_creates_nothing_and_selects_it(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = parish(cx, "t25-use");
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    open_panel(&shell, vcx);
    let before = song_count(&store, vcx);
    vcx.simulate_input("Tinapay sa Langit");
    vcx.run_until_parked();
    click(vcx, "button-3 possible existing Songs");
    click(vcx, "new-song-candidate-1");
    click(vcx, "button-Use this existing Song");
    settle_depth(&songs, vcx, 0);
    songs.read_with(vcx, |songs, _| {
        assert_eq!(songs.selected(), Some(ids.near));
        assert!(songs.new_song().is_none());
    });
    assert_eq!(song_count(&store, vcx), before);
    assert!(present(vcx, "songs-detail"));
    stop(vcx, &slot);
    let (slot, store) = reopened(cx, &dir);
    store.read_with(cx, |store, _| {
        assert_eq!(store.library().songs().count(), before)
    });
    cx.foreground_executor()
        .block_test(slot.shutdown())
        .expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn ac5_an_archived_candidate_turns_include_archived_on(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = parish(cx, "t25-archived");
    let (shell, vcx) = open_store(cx, store);
    let songs = songs_of(&shell, vcx);
    songs.read_with(vcx, |songs, _| assert!(!songs.query().include_archived));
    open_panel(&shell, vcx);
    vcx.simulate_input("Tinapay sa Langit");
    vcx.run_until_parked();
    click(vcx, "button-3 possible existing Songs");
    click(vcx, "new-song-candidate-2");
    click(vcx, "button-Use this existing Song");
    settle_depth(&songs, vcx, 0);
    songs.read_with(vcx, |songs, _| {
        assert!(songs.query().include_archived);
        assert_eq!(songs.selected(), Some(ids.archived));
        assert!(songs.hits().iter().any(|hit| hit.song == ids.archived));
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_create_commits_once_and_lands_on_the_new_song(cx: &mut TestAppContext) {
    let (dir, slot, store, _) = parish(cx, "t25-once");
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    open_panel(&shell, vcx);
    let before = song_count(&store, vcx);
    vcx.simulate_input("Kordero sa Dios");
    vcx.run_until_parked();
    // Double activation: the second click arrives before the write is answered.
    let bounds = vcx
        .debug_bounds("button-Create song")
        .expect("Create song is on screen");
    vcx.simulate_click(bounds.center(), Modifiers::default());
    vcx.simulate_click(bounds.center(), Modifiers::default());
    crate::songs::choirs::tests::wait(vcx, |cx| depth(&songs, cx) == 0);
    assert_eq!(song_count(&store, vcx), before + 1);
    let created = store.read_with(vcx, |store, _| {
        store
            .library()
            .songs()
            .find(|song| song.title() == "Kordero sa Dios")
            .map(SongWork::id)
            .expect("created")
    });
    songs.read_with(vcx, |songs, _| {
        assert_eq!(songs.selected(), Some(created));
        assert!(songs.new_song().is_none());
    });
    assert!(present(vcx, "songs-no-arrangements"));
    assert!(present(vcx, "button-Add first Arrangement"));
    // The action is shown and unavailable: activating it opens nothing.
    click(vcx, "button-Add first Arrangement");
    assert_eq!(depth(&songs, vcx), 0);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_enter_in_the_title_creates_the_song(cx: &mut TestAppContext) {
    let (dir, slot, store, _) = parish(cx, "t25-enter");
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    open_panel(&shell, vcx);
    let before = song_count(&store, vcx);
    vcx.simulate_input("Kordero sa Dios");
    vcx.simulate_keystrokes("enter");
    crate::songs::choirs::tests::wait(vcx, |cx| depth(&songs, cx) == 0);
    assert_eq!(song_count(&store, vcx), before + 1);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_a_storage_failure_keeps_the_panel_and_form_and_says_so(cx: &mut TestAppContext) {
    let store = cx.new(|cx| LibraryStore::with_library(memory().library, cx));
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    let panel = open_panel(&shell, vcx);
    let before = store.read_with(vcx, |store, _| store.library().clone());
    vcx.simulate_input("Kordero sa Dios");
    type_into(vcx, "new-song-composer", "Fr. Carlo Magno");
    let typed = form_state(&panel, vcx);
    click(vcx, "button-Create song");
    crate::songs::choirs::tests::wait(vcx, |cx| present(cx, "new-song-error"));
    assert_eq!(depth(&songs, vcx), 1);
    assert_eq!(form_state(&panel, vcx), typed);
    panel.read_with(vcx, |panel, _| {
        assert!(!panel.busy());
        assert_eq!(
            panel.error().map(ToString::to_string).as_deref(),
            Some("The parish store is not open.")
        );
    });
    store.read_with(vcx, |store, _| assert_eq!(store.library(), &before));
    songs.read_with(vcx, |songs, _| assert_eq!(songs.selected(), None));
    // Typing again clears the stale reason, and Create is available again.
    type_into(vcx, "new-song-composer", "!");
    assert!(!present(vcx, "new-song-error"));
    panel.read_with(vcx, |panel, cx| assert!(panel.can_create(cx)));
}

#[gpui::test]
fn ac6_cancel_and_escape_leave_no_record(cx: &mut TestAppContext) {
    let (dir, slot, store, _) = parish(cx, "t25-cancel");
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    let before = song_count(&store, vcx);
    open_panel(&shell, vcx);
    vcx.simulate_input("Never saved");
    vcx.run_until_parked();
    click(vcx, "button-Cancel");
    settle_depth(&songs, vcx, 0);
    open_panel(&shell, vcx);
    vcx.simulate_input("Never saved either");
    vcx.run_until_parked();
    vcx.simulate_keystrokes("escape");
    settle_depth(&songs, vcx, 0);
    songs.read_with(vcx, |songs, _| assert!(songs.new_song().is_none()));
    assert_eq!(song_count(&store, vcx), before);
    stop(vcx, &slot);
    let (slot, store) = reopened(cx, &dir);
    assert!(find_title(&store, "Never saved", cx).is_none());
    assert!(find_title(&store, "Never saved either", cx).is_none());
    cx.foreground_executor()
        .block_test(slot.shutdown())
        .expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn opening_and_cancelling_leaves_query_filters_selection_and_scroll_unchanged(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, ids) = parish(cx, "t25-unchanged");
    let (shell, vcx) = open_store(cx, store);
    let songs = songs_of(&shell, vcx);
    songs.update(vcx, |songs, cx| {
        songs.select(ids.unrelated, cx);
    });
    click(vcx, "songs-include-archived");
    songs.update(vcx, |songs, _| {
        songs.scroll_list_to(point(px(0.0), px(-4.0)))
    });
    vcx.run_until_parked();
    songs.update(vcx, |_, cx| cx.notify());
    settle(vcx);
    let snapshot = |songs: &SongsWorkspace| {
        (
            songs.query().clone(),
            songs.hits().to_vec(),
            songs.selected(),
            songs.list_offset(),
        )
    };
    let before = songs.read_with(vcx, |songs, _| snapshot(songs));
    open_panel(&shell, vcx);
    vcx.simulate_input("Tinapay sa Langit");
    vcx.run_until_parked();
    click(vcx, "button-Cancel");
    settle_depth(&songs, vcx, 0);
    assert_eq!(songs.read_with(vcx, |songs, _| snapshot(songs)), before);
    finish(vcx, &slot, &dir);
}

#[allow(dead_code)]
fn unused(_: SongLibrary) {}
