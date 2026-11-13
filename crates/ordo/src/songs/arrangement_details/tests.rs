//! Behavior of the Arrangement details panel through the shell that hosts it,
//! over a real store in a temporary directory.

use domain::{
    identity::{ArrangementId, ChoirId, SongId},
    song_library::{ArrangementDetails, ChoirDraft},
};
use gpui::{Entity, TestAppContext, VisualTestContext, px};
use storage::{Storage, StorageConfig};

use ui::primitives::Availability;

use super::{ArrangementDetailsPanel, Mode, Picker, build_details, form};
use crate::{
    shell::ShellView,
    songs::{
        LibraryStore,
        choirs::tests::{open_store, present, temp_dir, wait},
        new_song::tests::{
            click, finish, parish, parish_with, settle, settle_depth, songs_of, type_into,
        },
        store::tests::{hold_a_legacy_arrangement, hold_a_legacy_arrangement_in},
    },
};

/// Runs one store write to completion.
pub(crate) fn write<T: 'static>(
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

pub(crate) fn arrangement_count(
    store: &Entity<LibraryStore>,
    vcx: &mut VisualTestContext,
) -> usize {
    store.read_with(vcx, |store, _| store.library().arrangements().count())
}

pub(crate) fn choir_named(
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

pub(crate) fn label_of(
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
pub(crate) fn open_new(
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
pub(crate) fn open_edit(
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

#[gpui::test]
fn the_label_preview_follows_the_languages_then_the_first_choir_by_the_domains_rule(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, _) = parish(cx, "t26-preview");
    let (_shell, vcx) = open_store(cx, store.clone());
    let koro = choir_named(&store, "Koro Sto. Ni\u{f1}o", vcx).expect("koro");
    let youth = write(&store, vcx, |store, cx| {
        store.create_choir(ChoirDraft::named("Youth Choir"), cx)
    });
    let languages = ["Cebuano".to_owned(), "English".to_owned()];
    store.read_with(vcx, |store, _| {
        let library = store.library();
        // Nothing staged names the field, as the mockup does.
        assert_eq!(form::label_preview(library, &[], &[]), "Language");
        assert_eq!(
            form::label_preview(library, &[], &[koro]),
            "Language \u{b7} Koro Sto. Ni\u{f1}o"
        );
        assert_eq!(
            form::label_preview(library, &languages[..1], &[]),
            "Cebuano"
        );
        // The order the Choirs are staged in does not matter: Koro sorts first.
        for staged in [[koro, youth], [youth, koro]] {
            assert_eq!(
                form::label_preview(library, &languages, &staged),
                "Cebuano + English \u{b7} Koro Sto. Ni\u{f1}o +1"
            );
        }
    });
    finish(vcx, &slot, &dir);
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
    assert!(present(vcx, "overlay-New Arrangement"), "the New title");
    assert!(present(vcx, "button-Create Arrangement") && present(vcx, "button-Cancel"));
    assert!(!present(vcx, "button-Save details"));
    assert!(!present(vcx, "button-Duplicate Arrangement"));
    click(vcx, "button-Cancel");
    settle_depth(&songs_of(&shell, vcx), vcx, 0);

    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    let edit = open_edit(&shell, id, vcx);
    edit.read_with(vcx, |panel, _| assert_eq!(panel.mode(), Mode::Edit(id)));
    for field in fields {
        assert!(present(vcx, field), "{field} in Edit");
    }
    assert!(
        present(vcx, "overlay-Arrangement details"),
        "the Edit title"
    );
    assert!(present(vcx, "button-Save details") && present(vcx, "button-Duplicate Arrangement"));
    assert!(!present(vcx, "button-Create Arrangement"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac3_there_is_no_composer_field_and_the_other_details_are_saved(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-ac3");
    let (shell, vcx) = open_store(cx, store.clone());
    let before = arrangement_count(&store, vcx);
    open_new(&shell, seeded.near, vcx);
    // The form is exactly the six fields it renders by these labels, and no
    // label names a composer: the composer stays on the Song.
    assert_eq!(
        form::FIELD_LABELS,
        [
            "Display title",
            "Languages",
            "Choirs",
            "Translator or adapter",
            "Occasions",
            "Source and rights notes",
        ]
    );
    for label in form::FIELD_LABELS {
        assert!(!label.to_lowercase().contains("composer"), "{label}");
    }
    for wrapper in [
        "ad-title",
        "ad-languages",
        "ad-choirs",
        "ad-translator",
        "ad-occasions",
        "ad-notes",
    ] {
        assert!(present(vcx, wrapper), "{wrapper}");
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

    let panel = open_edit(&shell, shared, vcx);
    assert!(present(vcx, "ad-shared-notice"));
    panel.read_with(vcx, |panel, cx| {
        assert_eq!(
            panel.shared_notice(cx),
            Some(vec![
                "Koro Sto. Ni\u{f1}o".to_owned(),
                "Youth Choir".to_owned()
            ]),
            "the notice names both stored Choirs, in label order"
        );
    });
    // The notice follows what is stored, not what is staged.
    click(vcx, "button-Youth Choir");
    panel.read_with(vcx, |panel, _| assert_eq!(panel.choirs(), [koro]));
    assert!(present(vcx, "ad-shared-notice"));
    store.read_with(vcx, |store, _| {
        assert_eq!(
            store.library().arrangement(shared).unwrap().choirs().len(),
            2
        );
    });
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
        // The stored Arrangement was copied, not the staged edit (which
        // switched Youth Choir off), and the stage never reached the source.
        assert_eq!(source.choirs().len(), 2);
        assert!(source.choirs().contains(&koro) && source.choirs().contains(&youth));
        assert_eq!(duplicate.choirs(), source.choirs());
        assert_eq!(source.title(), Some("Shared"));
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
    store.read_with(vcx, |store, _| {
        assert_eq!(
            store.library().arrangement(shared).unwrap().choirs().len(),
            2
        );
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_every_way_out_stores_nothing_and_leaves_the_workspace_alone(cx: &mut TestAppContext) {
    // Long enough to scroll, so the scroll offset is something to keep.
    let (dir, slot, store, seeded) = parish_with(cx, "t26-ac5", 40, false);
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    click(vcx, "songs-search");
    vcx.simulate_input("a");
    vcx.run_until_parked();
    songs.update(vcx, |songs, cx| songs.select(seeded.near, cx));
    songs.update(vcx, |songs, _| {
        songs.scroll_list_to(gpui::point(px(0.0), px(-200.0)))
    });
    vcx.run_until_parked();
    songs.update(vcx, |_, cx| cx.notify());
    settle(vcx);
    let snapshot = |songs: &crate::songs::SongsWorkspace| {
        (
            songs.query().clone(),
            songs.hits().to_vec(),
            songs.selected(),
            songs.list_offset(),
        )
    };
    let before = songs.read_with(vcx, |songs, _| snapshot(songs));
    assert_eq!(before.2, Some(seeded.near));
    assert!(before.3.y < px(0.0), "the list is scrolled before opening");
    let arrangements = arrangement_count(&store, vcx);
    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    let details_before = store.read_with(vcx, |store, _| {
        store.library().arrangement(id).unwrap().details()
    });

    for edit in [false, true] {
        for way in ["Cancel", "close tab", "Escape"] {
            let panel = if edit {
                open_edit(&shell, id, vcx)
            } else {
                click(vcx, "button-Add Arrangement");
                songs.read_with(vcx, |songs, _| {
                    songs.new_arrangement().expect("open").clone()
                })
            };
            click(vcx, "button-Latin");
            panel.update(vcx, |panel, cx| panel.toggle_occasion("Lent", cx));
            match way {
                "Cancel" => click(vcx, "button-Cancel"),
                "close tab" => click(vcx, "button-Close"),
                _ => vcx.simulate_keystrokes("escape"),
            }
            settle_depth(&songs, vcx, 0);
            let which = format!("{} via {way}", if edit { "Edit" } else { "New" });
            assert_eq!(arrangement_count(&store, vcx), arrangements, "{which}");
            store.read_with(vcx, |store, _| {
                assert_eq!(
                    store.library().arrangement(id).unwrap().details(),
                    details_before,
                    "{which}"
                );
            });
            songs.read_with(vcx, |songs, _| {
                assert!(songs.new_arrangement().is_none(), "{which}");
                assert_eq!(snapshot(songs), before, "{which}: query, selection, scroll");
            });
            shell.read_with(vcx, |shell, _| {
                assert!(shell.arrangement_details().is_none(), "{which}");
            });
        }
    }
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_a_created_arrangement_opens_in_its_focused_view(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-create");
    let (shell, vcx) = open_store(cx, store.clone());
    let before = arrangement_count(&store, vcx);
    let known: Vec<ArrangementId> = store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangements()
            .map(domain::song_library::Arrangement::id)
            .collect()
    });
    open_new(&shell, seeded.near, vcx);
    click(vcx, "button-Latin");
    click(vcx, "button-Create Arrangement");
    wait(vcx, |vcx| arrangement_count(&store, vcx) == before + 1);
    settle(vcx);
    let focused = shell.read_with(vcx, |shell, _| shell.editing_arrangement());
    let focused = focused.expect("the new Arrangement is open");
    assert!(!known.contains(&focused), "the focused view is the new one");
    assert_eq!(label_of(&store, focused, vcx), "Latin");
    assert!(present(vcx, "focused-editor") && present(vcx, "arrangement-overview"));
    assert!(present(vcx, "arrangement-no-sections"));
    assert!(present(vcx, "button-Add first section"));
    assert!(
        !present(vcx, "control-unavailable"),
        "adding is available now"
    );
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
fn a_double_activation_of_the_primary_action_writes_once(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-double");
    let (shell, vcx) = open_store(cx, store.clone());
    let panel = open_new(&shell, seeded.near, vcx);
    click(vcx, "button-Latin");
    let before = arrangement_count(&store, vcx);
    // Both inside one update: the write cannot be answered between them, and a
    // second click cannot land on the workspace underneath.
    vcx.update(|window, cx| {
        panel.update(cx, |panel, cx| {
            panel.submit(window, cx);
            panel.submit(window, cx);
        });
    });
    wait(vcx, |vcx| arrangement_count(&store, vcx) > before);
    settle(vcx);
    assert_eq!(arrangement_count(&store, vcx), before + 1);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn the_primary_action_waits_for_a_choir_that_is_still_being_created(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-choir-busy");
    let (shell, vcx) = open_store(cx, store.clone());
    let panel = open_new(&shell, seeded.near, vcx);
    click(vcx, "button-Latin");
    let before = arrangement_count(&store, vcx);
    // Create the Choir and press the primary action before the Choir write is
    // answered: nothing is stored without the Choir.
    vcx.update(|window, cx| {
        panel.update(cx, |panel, cx| {
            panel.create_choir("Late Choir".to_owned(), window, cx);
            assert_eq!(
                panel.primary_availability(),
                Availability::unavailable(form::CHOIR_BUSY_REASON)
            );
            panel.submit(window, cx);
            assert!(!panel.busy(), "submit did nothing");
        });
    });
    wait(vcx, |vcx| choir_named(&store, "Late Choir", vcx).is_some());
    settle(vcx);
    let made = choir_named(&store, "Late Choir", vcx).expect("saved");
    assert_eq!(arrangement_count(&store, vcx), before);
    panel.read_with(vcx, |panel, _| {
        assert_eq!(panel.choirs(), [made]);
        assert_eq!(panel.primary_availability(), Availability::Available);
    });
    click(vcx, "button-Create Arrangement");
    wait(vcx, |vcx| arrangement_count(&store, vcx) == before + 1);
    settle(vcx);
    let created = shell
        .read_with(vcx, |shell, _| shell.editing_arrangement())
        .expect("open");
    store.read_with(vcx, |store, _| {
        assert_eq!(
            store.library().arrangement(created).unwrap().choirs().len(),
            1
        );
    });
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

/// The revisions an Arrangement has, oldest first.
fn revisions_of(
    slot: &crate::songs::StorageSlot,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) -> Vec<storage::RevisionRecord> {
    let future = slot
        .with(|storage| storage.revisions(domain::revisions::RevisionEntity::Arrangement(id)))
        .expect("storage is open");
    vcx.foreground_executor()
        .block_test(future)
        .expect("revisions")
}

#[gpui::test]
fn a_save_with_no_change_writes_nothing_shows_no_toast_and_closes(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-nochange");
    let (shell, vcx) = open_store(cx, store.clone());
    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    let revisions = revisions_of(&slot, id, vcx).len();
    let panel = open_edit(&shell, id, vcx);
    let stored = store.read_with(vcx, |store, _| {
        store.library().arrangement(id).expect("stored").details()
    });
    assert!(details_of(&panel, vcx).same_as(&stored));
    click(vcx, "button-Save details");
    settle_depth(&songs_of(&shell, vcx), vcx, 0);
    shell.read_with(vcx, |shell, cx| {
        assert!(
            shell.overlays().toasts(cx).is_empty(),
            "no toast for no change"
        );
    });
    assert_eq!(
        revisions_of(&slot, id, vcx).len(),
        revisions,
        "no new revision"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn a_failed_save_shows_the_stores_reason_and_keeps_the_form(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-save-fails");
    let (shell, vcx) = open_store(cx, store.clone());
    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    let panel = open_edit(&shell, id, vcx);
    panel.update(vcx, |panel, cx| panel.toggle_occasion("Lent", cx));
    let staged = details_of(&panel, vcx);
    let library = store.read_with(vcx, |store, _| store.library().clone());

    // Swap in an open store of another, empty parish: memory still holds the
    // Arrangement, the domain accepts the edit, and storage cannot find it.
    let outcome = vcx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
    let other = temp_dir("t26-other-parish");
    slot.put(Storage::spawn(StorageConfig::in_directory(&other)));

    click(vcx, "button-Save details");
    wait(vcx, |vcx| {
        panel.read_with(vcx, |panel, _| panel.error().is_some())
    });
    settle(vcx);
    panel.read_with(vcx, |panel, _| {
        let reason = panel.error().expect("the reason is shown");
        assert!(reason.contains("was not found"), "{reason}");
        assert!(!panel.busy());
    });
    assert!(present(vcx, "ad-error"), "the failure is drawn");
    shell.read_with(vcx, |shell, cx| {
        assert!(
            shell.arrangement_details().is_some(),
            "the surface stays open"
        );
        assert!(shell.overlays().toasts(cx).is_empty(), "no success toast");
    });
    assert_eq!(details_of(&panel, vcx), staged, "the staged form is kept");
    store.read_with(vcx, |store, _| assert_eq!(store.library(), &library));
    click(vcx, "button-Cancel");
    settle_depth(&songs_of(&shell, vcx), vcx, 0);
    finish(vcx, &slot, &other);
    let _ = std::fs::remove_dir_all(&dir);
}

/// T2.6a rule 1: a language the Arrangement holds that is not in the built-in
/// list is a selected chip in Edit, can be switched back on, and survives Save.
#[gpui::test]
fn a_stored_language_that_is_not_listed_is_a_chip_in_edit_and_survives_save(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, seeded) = parish(cx, "t26-unlisted");
    let (shell, vcx) = open_store(cx, store.clone());
    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    hold_a_legacy_arrangement_in(
        vcx,
        &store,
        seeded.near,
        id,
        vec!["Latin".to_owned(), "ceb".to_owned()],
    );
    settle(vcx);
    let panel = open_edit(&shell, id, vcx);
    assert!(present(vcx, "button-ceb"), "the held language is a chip");
    panel.read_with(vcx, |panel, _| {
        assert_eq!(panel.languages(), ["Latin", "ceb"])
    });
    // Switching it off keeps the chip, so it can be switched back on.
    click(vcx, "button-ceb");
    panel.read_with(vcx, |panel, _| assert_eq!(panel.languages(), ["Latin"]));
    assert!(
        present(vcx, "button-ceb"),
        "still offered after switching off"
    );
    click(vcx, "button-ceb");
    panel.read_with(vcx, |panel, _| {
        assert_eq!(panel.languages(), ["Latin", "ceb"])
    });
    panel.update(vcx, |panel, cx| panel.toggle_occasion("Advent", cx));
    click(vcx, "button-Save details");
    wait(vcx, |vcx| {
        store.read_with(vcx, |store, _| {
            store
                .library()
                .arrangement(id)
                .unwrap()
                .occasions()
                .contains("Advent")
        })
    });
    settle(vcx);
    store.read_with(vcx, |store, _| {
        assert_eq!(
            store.library().arrangement(id).unwrap().languages(),
            ["Latin", "ceb"]
        );
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn the_header_line_names_the_song_and_in_edit_the_label(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-header");
    let (_shell, vcx) = open_store(cx, store.clone());
    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    store.read_with(vcx, |store, _| {
        let library = store.library();
        let song = library.song(seeded.near).unwrap().title().to_owned();
        let label = library.arrangement_label(id).unwrap();
        assert_eq!(
            ArrangementDetailsPanel::header_line(library, Mode::New(seeded.near)),
            song
        );
        assert_eq!(
            ArrangementDetailsPanel::header_line(library, Mode::Edit(id)),
            format!("{song} \u{b7} {label}")
        );
    });
    finish(vcx, &slot, &dir);
}

/// B1: a parish stored before T2.6 may hold an Arrangement with no language.
/// A copy is a creation, so Duplicate is shown blocked with its reason rather
/// than failing with the domain's raw refusal, and pressing it writes nothing.
#[gpui::test]
fn duplicate_is_blocked_with_a_reason_for_an_arrangement_with_no_language(cx: &mut TestAppContext) {
    let (dir, slot, store, seeded) = parish(cx, "t26-dup-legacy");
    let (shell, vcx) = open_store(cx, store.clone());
    let id = arrangement_of(&store, seeded.near, "Cebuano unison", vcx);
    hold_a_legacy_arrangement(vcx, &store, seeded.near, id);
    settle(vcx);
    let panel = open_edit(&shell, id, vcx);
    panel.read_with(vcx, |panel, cx| {
        assert_eq!(
            panel.duplicate_availability(cx),
            Availability::unavailable(form::DUPLICATE_NO_LANGUAGE_REASON)
        );
    });
    assert!(present(vcx, "button-Duplicate Arrangement") && present(vcx, "control-unavailable"));

    // The reason wraps inside the slideover and pushes Cancel and Save below
    // it: nothing overflows the surface or overlaps another action.
    let bounds = |vcx: &mut VisualTestContext, selector: &'static str| {
        vcx.debug_bounds(selector).expect("on screen")
    };
    let card = bounds(vcx, "overlay-Arrangement details");
    let reason = bounds(vcx, "control-unavailable");
    let duplicate = bounds(vcx, "button-Duplicate Arrangement");
    let cancel = bounds(vcx, "button-Cancel");
    for inner in [reason, duplicate, cancel] {
        assert!(inner.left() >= card.left() && inner.right() <= card.right());
    }
    assert!(cancel.top() >= reason.bottom());
    let before = arrangement_count(&store, vcx);
    click(vcx, "button-Duplicate Arrangement");
    settle(vcx);
    assert_eq!(arrangement_count(&store, vcx), before, "nothing is written");
    // The panel stays open with no error: the block is not a failed write.
    shell.read_with(vcx, |shell, _| {
        assert!(shell.arrangement_details().is_some())
    });
    panel.read_with(vcx, |panel, _| assert!(panel.error.is_none()));
    click(vcx, "button-Cancel");
    finish(vcx, &slot, &dir);
}
