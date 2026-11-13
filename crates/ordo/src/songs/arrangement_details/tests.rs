//! Behavior of the Arrangement details panel through the shell that hosts it,
//! over a real store in a temporary directory.

use domain::{
    identity::{ArrangementId, ChoirId, SongId},
    song_library::{ArrangementDetails, ChoirDraft},
};
use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{ArrangementDetailsPanel, Mode, Picker, build_details, derived_label};
use crate::{
    shell::ShellView,
    songs::{
        LibraryStore,
        choirs::tests::{open_store, present, wait},
        new_song::tests::{click, finish, parish, settle, settle_depth, songs_of, type_into},
    },
};

/// Runs one store write to completion.
fn write<T: 'static>(
    store: &Entity<LibraryStore>,
    vcx: &mut VisualTestContext,
    write: impl FnOnce(
        &mut LibraryStore,
        &mut gpui::Context<LibraryStore>,
    ) -> gpui::Task<Result<T, gpui::SharedString>>,
) -> T {
    let task = store.update(vcx, write);
    vcx.foreground_executor()
        .block_test(task)
        .expect("the write succeeds")
}

fn arrangement_count(store: &Entity<LibraryStore>, vcx: &mut VisualTestContext) -> usize {
    store.read_with(vcx, |store, _| store.library().arrangements().count())
}

fn choir_named(
    store: &Entity<LibraryStore>,
    name: &str,
    vcx: &mut VisualTestContext,
) -> Option<ChoirId> {
    store.read_with(vcx, |store, _| {
        store
            .library()
            .choirs()
            .find(|choir| choir.name() == name)
            .map(domain::song_library::Choir::id)
    })
}

fn label_of(
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) -> String {
    store.read_with(vcx, |store, _| {
        store.library().arrangement_label(id).expect("label")
    })
}

fn arrangement_of(
    store: &Entity<LibraryStore>,
    song: SongId,
    label: &str,
    vcx: &mut VisualTestContext,
) -> ArrangementId {
    store.read_with(vcx, |store, _| {
        let library = store.library();
        library
            .arrangements_for_song(song)
            .expect("song")
            .into_iter()
            .map(domain::song_library::Arrangement::id)
            .find(|id| library.arrangement_label(*id).as_deref() == Ok(label))
            .expect("arrangement")
    })
}

/// Selects `song` and opens New Arrangement from its detail pane.
fn open_new(
    shell: &Entity<ShellView>,
    song: SongId,
    vcx: &mut VisualTestContext,
) -> Entity<ArrangementDetailsPanel> {
    let songs = songs_of(shell, vcx);
    songs.update(vcx, |songs, cx| songs.select(song, cx));
    settle(vcx);
    click(vcx, "button-Add Arrangement");
    songs.read_with(vcx, |songs, _| {
        songs.new_arrangement().expect("the panel is open").clone()
    })
}

/// Opens the focused view of `id` and then its details panel.
fn open_edit(
    shell: &Entity<ShellView>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) -> Entity<ArrangementDetailsPanel> {
    shell.update(vcx, |shell, cx| shell.open_arrangement(id, cx));
    settle(vcx);
    click(vcx, "button-Arrangement details");
    shell.read_with(vcx, |shell, _| {
        shell
            .arrangement_details()
            .expect("the panel is open")
            .clone()
    })
}

fn details_of(
    panel: &Entity<ArrangementDetailsPanel>,
    vcx: &mut VisualTestContext,
) -> ArrangementDetails {
    panel.read_with(vcx, |panel, cx| panel.details(cx))
}

#[test]
fn the_label_preview_follows_the_languages_then_the_first_choir() {
    let none: [String; 0] = [];
    assert_eq!(derived_label(&none, &none), "Language");
    let languages = ["Cebuano".to_owned(), "English".to_owned()];
    let choirs = ["Koro".to_owned(), "Youth".to_owned(), "Zeal".to_owned()];
    assert_eq!(derived_label(&languages[..1], &none), "Cebuano");
    assert_eq!(
        derived_label(&languages, &choirs[..1]),
        "Cebuano + English \u{b7} Koro"
    );
    assert_eq!(
        derived_label(&languages, &choirs),
        "Cebuano + English \u{b7} Koro +2"
    );
}

#[test]
fn blank_texts_are_no_value_and_authored_ones_are_kept() {
    let details = build_details("  ", &[], &[], "", &[], " ");
    assert_eq!(details.title, None);
    assert_eq!(details.rendition_attribution, None);
    assert_eq!(details.notes, None);
    let details = build_details(
        " T ",
        &["Latin".to_owned()],
        &[],
        "Fr. X",
        &["Lent".to_owned()],
        "n",
    );
    assert_eq!(details.title.as_deref(), Some(" T "));
    assert_eq!(details.rendition_attribution.as_deref(), Some("Fr. X"));
}

#[gpui::test]
fn ac1_new_and_edit_open_the_same_surface_with_create_and_save_actions(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-ac1");
    let (shell, vcx) = open_store(cx, store.clone());
    let new = open_new(&shell, seeded.near, vcx);
    new.read_with(vcx, |panel, _| {
        assert_eq!(panel.mode(), Mode::New(seeded.near));
    });
    let fields = [
        "ad-title",
        "ad-languages",
        "ad-choirs",
        "ad-translator",
        "ad-occasions",
        "ad-notes",
    ];
    for field in fields {
        assert!(present(vcx, field), "{field} in New");
    }
    assert!(present(vcx, "button-Create Arrangement") && present(vcx, "button-Cancel"));
    assert!(!present(vcx, "button-Duplicate Arrangement"));
    click(vcx, "button-Cancel");
    settle_depth(&songs_of(&shell, vcx), vcx, 0);

    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    let edit = open_edit(&shell, id, vcx);
    edit.read_with(vcx, |panel, _| assert_eq!(panel.mode(), Mode::Edit(id)));
    for field in fields {
        assert!(present(vcx, field), "{field} in Edit");
    }
    assert!(present(vcx, "button-Save details") && present(vcx, "button-Duplicate Arrangement"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac3_there_is_no_composer_field_and_the_other_details_are_saved(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-ac3");
    let (shell, vcx) = open_store(cx, store.clone());
    let before = arrangement_count(&store, vcx);
    open_new(&shell, seeded.near, vcx);
    for absent in ["ad-composer", "ad-author", "ad-original-author"] {
        assert!(!present(vcx, absent), "{absent}");
    }
    click(vcx, "button-Latin");
    click(vcx, "button-Advent");
    type_into(vcx, "ad-translator", "Fr. Adapter");
    type_into(vcx, "ad-notes", "Public domain");
    click(vcx, "button-Create Arrangement");
    wait(vcx, |vcx| arrangement_count(&store, vcx) == before + 1);
    settle(vcx);
    let id = shell.read_with(vcx, |shell, _| {
        shell.editing_arrangement().expect("focused")
    });
    store.read_with(vcx, |store, _| {
        let arrangement = store.library().arrangement(id).expect("stored");
        assert_eq!(arrangement.languages(), ["Latin"]);
        assert_eq!(arrangement.rendition_attribution(), Some("Fr. Adapter"));
        assert_eq!(arrangement.notes(), Some("Public domain"));
        assert!(arrangement.occasions().contains("Advent"));
        assert_eq!(arrangement.title(), None);
    });
    assert_eq!(label_of(&store, id, vcx), "Latin");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac4_the_shared_notice_shows_only_for_more_than_one_stored_choir_and_duplicate_is_independent(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, seeded) = parish(cx, "t26-ac4");
    let (shell, vcx) = open_store(cx, store.clone());
    let koro = choir_named(&store, "Koro Sto. Ni\u{f1}o", vcx).expect("koro");
    let youth = write(&store, vcx, |store, cx| {
        store.create_choir(ChoirDraft::named("Youth Choir"), cx)
    });
    let single = arrangement_of(&store, seeded.exact, "Cebuano SATB", vcx);
    let shared = write(&store, vcx, |store, cx| {
        store.create_arrangement(
            seeded.exact,
            build_details(
                "Shared",
                &["Cebuano".to_owned()],
                &[koro, youth],
                "",
                &[],
                "",
            ),
            cx,
        )
    });
    open_edit(&shell, single, vcx);
    assert!(!present(vcx, "ad-shared-notice"));
    click(vcx, "button-Cancel");
    settle_depth(&songs_of(&shell, vcx), vcx, 0);

    open_edit(&shell, shared, vcx);
    assert!(present(vcx, "ad-shared-notice"));
    // The notice follows what is stored, not what is staged.
    click(vcx, "button-Youth Choir");
    assert!(present(vcx, "ad-shared-notice"));
    let before = arrangement_count(&store, vcx);
    click(vcx, "button-Duplicate Arrangement");
    wait(vcx, |vcx| arrangement_count(&store, vcx) == before + 1);
    settle(vcx);
    let copy = shell.read_with(vcx, |shell, _| {
        shell.editing_arrangement().expect("focused")
    });
    assert_ne!(copy, shared);
    store.read_with(vcx, |store, _| {
        let library = store.library();
        let source = library.arrangement(shared).expect("source");
        let duplicate = library.arrangement(copy).expect("copy");
        assert_eq!(duplicate.song(), source.song());
        assert_eq!(duplicate.languages(), source.languages());
        // The stored Arrangement was copied, not the staged edit.
        assert_eq!(duplicate.choirs(), source.choirs());
    });
    // Editing the copy leaves the source alone.
    shell.read_with(vcx, |shell, _| {
        assert!(shell.arrangement_details().is_none());
    });
    click(vcx, "button-Arrangement details");
    click(vcx, "ad-title");
    vcx.simulate_keystrokes("ctrl-a");
    vcx.simulate_input("Copy title");
    vcx.run_until_parked();
    click(vcx, "button-Save details");
    wait(vcx, |vcx| label_of(&store, copy, vcx) == "Copy title");
    assert_eq!(label_of(&store, shared, vcx), "Shared");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_cancel_and_escape_store_nothing_and_leave_the_workspace_alone(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-ac5");
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    songs.update(vcx, |songs, cx| songs.select(seeded.near, cx));
    settle(vcx);
    let before = arrangement_count(&store, vcx);
    let query = songs.read_with(vcx, |songs, _| songs.query().clone());
    for dismiss in ["cancel", "escape"] {
        open_new(&shell, seeded.near, vcx);
        click(vcx, "button-Latin");
        type_into(vcx, "ad-notes", "staged");
        if dismiss == "cancel" {
            click(vcx, "button-Cancel");
        } else {
            vcx.simulate_keystrokes("escape");
        }
        settle_depth(&songs, vcx, 0);
        assert_eq!(arrangement_count(&store, vcx), before, "{dismiss}");
        songs.read_with(vcx, |songs, _| {
            assert!(songs.new_arrangement().is_none());
            assert_eq!(songs.selected(), Some(seeded.near));
            assert_eq!(songs.query(), &query);
        });
    }
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_a_created_arrangement_opens_in_its_focused_view(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-create");
    let (shell, vcx) = open_store(cx, store.clone());
    let before = arrangement_count(&store, vcx);
    open_new(&shell, seeded.near, vcx);
    click(vcx, "button-Latin");
    click(vcx, "button-Create Arrangement");
    wait(vcx, |vcx| arrangement_count(&store, vcx) == before + 1);
    settle(vcx);
    assert!(present(vcx, "focused-editor") && present(vcx, "arrangement-overview"));
    assert!(present(vcx, "arrangement-no-sections"));
    assert!(present(vcx, "button-Add first section") && present(vcx, "control-unavailable"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn create_with_no_language_shows_the_error_and_stores_nothing(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-nolang");
    let (shell, vcx) = open_store(cx, store.clone());
    let panel = open_new(&shell, seeded.near, vcx);
    let before = arrangement_count(&store, vcx);
    assert!(!present(vcx, "ad-language-error"));
    click(vcx, "button-Create Arrangement");
    settle(vcx);
    assert!(present(vcx, "ad-language-error"));
    assert_eq!(arrangement_count(&store, vcx), before);
    panel.read_with(vcx, |panel, _| {
        assert!(!panel.busy() && panel.error().is_none());
    });
    assert!(present(vcx, "ad-form"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn a_double_click_on_the_primary_action_writes_once(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-double");
    let (shell, vcx) = open_store(cx, store.clone());
    open_new(&shell, seeded.near, vcx);
    click(vcx, "button-Latin");
    let before = arrangement_count(&store, vcx);
    let bounds = vcx
        .debug_bounds("button-Create Arrangement")
        .expect("button");
    vcx.simulate_click(bounds.center(), gpui::Modifiers::default());
    vcx.simulate_click(bounds.center(), gpui::Modifiers::default());
    wait(vcx, |vcx| arrangement_count(&store, vcx) > before);
    settle(vcx);
    assert_eq!(arrangement_count(&store, vcx), before + 1);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn the_language_picker_searches_the_built_in_list_with_no_create_row(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-lang");
    let (shell, vcx) = open_store(cx, store.clone());
    let panel = open_new(&shell, seeded.near, vcx);
    click(vcx, "button-Language");
    settle(vcx);
    panel.read_with(vcx, |panel, _| {
        assert_eq!(panel.picker(), Some(Picker::Language));
    });
    assert!(present(vcx, "ad-picker-language"));
    vcx.simulate_input("ilong");
    vcx.run_until_parked();
    settle(vcx);
    assert!(present(vcx, "ad-pick-row-0"));
    assert!(!present(vcx, "ad-pick-row-1"));
    vcx.simulate_input("zzzz");
    vcx.run_until_parked();
    settle(vcx);
    assert!(present(vcx, "ad-pick-none") && !present(vcx, "ad-pick-row-0"));
    // Escape closes the picker first; the panel stays.
    vcx.simulate_keystrokes("escape");
    settle(vcx);
    panel.read_with(vcx, |panel, _| {
        assert_eq!(panel.picker(), None, "picker state")
    });
    assert!(!present(vcx, "ad-picker-language"), "picker drawn");
    assert!(present(vcx, "ad-form"), "form gone");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn the_choir_picker_never_offers_an_archived_choir_and_a_created_choir_persists_after_cancel(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, seeded) = parish(cx, "t26-choir");
    let (shell, vcx) = open_store(cx, store.clone());
    let old = write(&store, vcx, |store, cx| {
        store.create_choir(ChoirDraft::named("Old Choir"), cx)
    });
    write(&store, vcx, |store, cx| store.archive_choir(old, cx));
    let panel = open_new(&shell, seeded.near, vcx);
    assert!(!present(vcx, "button-Old Choir"));
    click(vcx, "button-Choir");
    settle(vcx);
    vcx.simulate_input("Old Choir");
    vcx.run_until_parked();
    settle(vcx);
    // The archived name is not suggested, and exists, so nothing is created.
    assert!(!present(vcx, "ad-pick-row-0"));
    click(vcx, "button-Done");
    click(vcx, "button-Choir");
    vcx.simulate_input("Brand New Choir");
    vcx.run_until_parked();
    settle(vcx);
    assert!(present(vcx, "ad-pick-row-0"));
    vcx.simulate_keystrokes("enter");
    wait(vcx, |vcx| {
        choir_named(&store, "Brand New Choir", vcx).is_some()
    });
    settle(vcx);
    let made = choir_named(&store, "Brand New Choir", vcx).expect("saved at once");
    panel.read_with(vcx, |panel, _| assert_eq!(panel.choirs(), [made]));
    assert!(present(vcx, "ad-choir-made"));
    let before = arrangement_count(&store, vcx);
    click(vcx, "button-Cancel");
    settle_depth(&songs_of(&shell, vcx), vcx, 0);
    assert_eq!(arrangement_count(&store, vcx), before);
    assert_eq!(choir_named(&store, "Brand New Choir", vcx), Some(made));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn a_save_with_no_change_writes_nothing_and_closes(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-nochange");
    let (shell, vcx) = open_store(cx, store.clone());
    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    let panel = open_edit(&shell, id, vcx);
    let staged = details_of(&panel, vcx);
    store.read_with(vcx, |store, _| {
        assert_eq!(
            store.library().arrangement(id).expect("stored").details(),
            staged
        );
    });
    click(vcx, "button-Save details");
    settle_depth(&songs_of(&shell, vcx), vcx, 0);
    finish(vcx, &slot, &dir);
}
