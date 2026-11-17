//! Owner lab for the T2.4 Songs and Choirs workspaces, over a real parish.
//!
//! `cargo run -p ordo --example songs-library-lab` seeds a fresh parish folder
//! `ordo-songs-library-lab-<pid>` under the system temporary directory through
//! the storage Module Interface: a Cebuano and English mix of Songs and
//! Arrangements, three Choirs, an archived Song and an archived Choir. It then
//! opens the real operator window over it, wired as the application wires its
//! parish store: an app-level `StorageSlot` that stops the worker on quit and a
//! `LibraryStore` opened off the GPUI thread. Open Library to try search,
//! filters, the Songs round trip and the Choirs workspace.
//!
//! Installation preferences and the parish live in that one folder, so the lab
//! never reads or writes the real Parish data. The folder is printed and left
//! in place; each run uses its own process id and an existing folder is refused.

use std::{
    future::Future,
    path::Path,
    pin::pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread::{self, Thread},
};

use gpui::{App, AppContext as _};
use gpui_platform::application;
use ordo::{
    open_native_runtime_at,
    songs::{LibraryStore, StorageSlot},
};
use storage::{
    NewArrangement, NewChoir, NewSection, NewSong, ShutdownOutcome, Storage, StorageConfig,
};

/// Unparks the thread blocked in [`block_on`].
struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Parks the calling thread until `future` completes; seeding runs before any
/// window exists, so there is no GPUI thread to protect.
fn block_on<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        thread::park();
    }
}

type Failure = Box<dyn std::error::Error>;

/// A title with two possible existing Songs in the seeded library.
const NEAR_TITLE: &str = "Tinapay sa Langit";

/// One Arrangement to seed: title, language, Choir indexes, occasions, lyrics.
struct Setting {
    title: &'static str,
    language: &'static str,
    choirs: &'static [usize],
    occasions: &'static [&'static str],
    verse: &'static str,
    chorus: &'static str,
}

/// One Song to seed, with its Arrangements.
struct Work {
    title: &'static str,
    aliases: &'static [&'static str],
    attribution: Option<&'static str>,
    archived: bool,
    settings: &'static [Setting],
}

const WORKS: &[Work] = &[
    Work {
        title: "Tinapay sa Langit",
        aliases: &["Bread from Heaven"],
        attribution: Some("Fr. Manoling Francisco"),
        archived: false,
        settings: &[
            Setting {
                title: "Cebuano SATB",
                language: "Cebuano",
                choirs: &[0],
                occasions: &["Holy Thursday"],
                verse: "Tinapay sa langit among kalipay",
                chorus: "Dayandayan ang gugma sa Ginoo",
            },
            Setting {
                title: "English SATB",
                language: "English",
                choirs: &[1],
                occasions: &["Holy Thursday"],
                verse: "Bread from heaven, our joy and our life",
                chorus: "Forever the love of the Lord",
            },
        ],
    },
    // A near match of "Tinapay sa Langit" (different punctuation and spacing),
    // so that title has two possible existing Songs in the New song panel.
    Work {
        title: "Tinapay, sa Langit",
        aliases: &[],
        attribution: Some("fr manoling francisco"),
        archived: false,
        settings: &[Setting {
            title: "Cebuano unison",
            language: "Cebuano",
            choirs: &[],
            occasions: &[],
            verse: "Tinapay, sa langit",
            chorus: "Dayandayan",
        }],
    },
    Work {
        title: "Ave Maria",
        aliases: &[],
        attribution: Some("Franz Schubert"),
        archived: false,
        settings: &[Setting {
            title: "Latin SATB",
            language: "Latin",
            choirs: &[0, 2],
            occasions: &["Marian feasts"],
            verse: "Ave Maria, gratia plena",
            chorus: "Sancta Maria, mater Dei",
        }],
    },
    Work {
        title: "Gloria sa Kahitas-an",
        aliases: &[],
        attribution: None,
        archived: false,
        settings: &[
            Setting {
                title: "Cebuano unison",
                language: "Cebuano",
                choirs: &[0, 1],
                occasions: &["Christmas"],
                verse: "Himaya sa Dios sa kahitas-an",
                chorus: "Ug sa yuta, pagpakig-uli",
            },
            Setting {
                title: "English SATB",
                language: "English",
                choirs: &[2],
                occasions: &["Christmas"],
                verse: "Glory to God in the highest",
                chorus: "And peace to his people on earth",
            },
        ],
    },
    Work {
        title: "Ang Ginoo Akong Magbalantay",
        aliases: &["The Lord Is My Shepherd"],
        attribution: None,
        archived: false,
        settings: &[Setting {
            title: "Responsorial Psalm 23",
            language: "Cebuano",
            choirs: &[],
            occasions: &[],
            verse: "Ang Ginoo akong magbalantay",
            chorus: "Wala akoy kulang",
        }],
    },
    Work {
        title: "Old Processional",
        aliases: &[],
        attribution: None,
        archived: true,
        settings: &[Setting {
            title: "English SATB",
            language: "English",
            choirs: &[0],
            occasions: &[],
            verse: "Of old we walked in line",
            chorus: "Come, follow on",
        }],
    },
];

/// Choir names, and whether each is archived.
const CHOIRS: &[(&str, bool)] = &[
    ("Koro Sto. Niño", false),
    ("Youth Choir", false),
    ("San Roque Singers", false),
    ("Old Parish Choir", true),
];

/// Seeds the parish and shuts the storage worker down cleanly.
fn seed(dir: &Path) -> Result<(), Failure> {
    let storage = Storage::spawn(StorageConfig::in_directory(dir));
    let mut choirs = Vec::new();
    for (name, archived) in CHOIRS {
        let record = block_on(storage.create_choir(NewChoir {
            name: (*name).to_owned(),
            short_label: None,
            languages: Vec::new(),
            notes: None,
        }))?;
        if *archived {
            block_on(storage.archive_choir(record.id))?;
        }
        choirs.push(record.id);
    }
    for work in WORKS {
        let song = block_on(
            storage.create_song(NewSong {
                title: work.title.to_owned(),
                aliases: work
                    .aliases
                    .iter()
                    .map(|alias| (*alias).to_owned())
                    .collect(),
                original_attribution: work.attribution.map(str::to_owned),
                provenance: None,
                rights_notes: None,
                liturgical_moments: Vec::new(),
                occasions: Vec::new(),
            }),
        )?;
        for setting in work.settings {
            block_on(
                storage.create_arrangement(NewArrangement {
                    song: song.id,
                    title: Some(setting.title.to_owned()),
                    notes: None,
                    languages: vec![setting.language.to_owned()],
                    choirs: setting.choirs.iter().map(|at| choirs[*at]).collect(),
                    rendition_attribution: None,
                    occasions: setting
                        .occasions
                        .iter()
                        .map(|text| (*text).to_owned())
                        .collect(),
                    sections: vec![
                        NewSection {
                            label: Some("Verse 1".to_owned()),
                            lyrics: setting.verse.to_owned(),
                        },
                        NewSection {
                            label: Some("Chorus".to_owned()),
                            lyrics: setting.chorus.to_owned(),
                        },
                    ],
                    sequence: vec![0, 1, 0],
                }),
            )?;
        }
        if work.archived {
            block_on(storage.archive_song(song.id))?;
        }
    }
    match block_on(storage.shutdown()) {
        ShutdownOutcome::Clean => Ok(()),
        other => Err(format!("the seeding store did not stop cleanly: {other:?}").into()),
    }
}

fn main() -> Result<(), Failure> {
    let dir = std::env::temp_dir().join(format!("ordo-songs-library-lab-{}", std::process::id()));
    if dir.exists() {
        return Err(format!("{} already exists; remove it and rerun", dir.display()).into());
    }
    std::fs::create_dir_all(&dir)?;
    seed(&dir)?;
    println!("Lab parish folder: {}", dir.display());
    println!("Library > Songs and Library > Choirs read it; nothing else is touched.");
    println!("New song path: Library > Songs > New song. Type the title {NEAR_TITLE:?}:");
    println!("  - \"2 possible existing Songs\" appears under the title, collapsed.");
    println!("  - Expand it, choose a candidate: a preview opens over the form (identity only).");
    println!(
        "  - Back returns to the form exactly as typed; Use this existing Song selects it instead."
    );
    println!("  - Create song adds the Song; its detail shows No Arrangements yet.");
    println!("Arrangement path: select a Song > Add Arrangement (or Add first Arrangement):");
    println!("  - Pick two languages and two Choirs, leave the title blank: the label preview");
    println!("    follows the languages, then the first Choir. Create Arrangement opens it.");
    println!("  - Arrangement details on a shared Arrangement shows the shared notice;");
    println!("    Save details stores the edit, Cancel (or Escape) discards it.");
    println!("  - Duplicate Arrangement copies the stored Arrangement and opens the copy.");
    println!("Lyrics path: open a new Arrangement (no sections yet), then by keyboard only:");
    println!("  - Enter on Add first section, type Verse 1 over the suggestion, Enter: the lyric");
    println!("    editor has focus. Type four lines, press Enter twice after line 2 for a new");
    println!(
        "    slide, Backspace to join it again; thumbnails and the Inspector follow the caret."
    );
    println!("  - Split after line 2 and Join slide 2 with slide 1 do the same with the mouse.");
    println!("  - F6 to the sections pane, Add section, type Chorus, Enter, type its lines.");
    println!(
        "  - Focus the Chorus row and press Alt+Up: it moves first; the Sequence is unchanged."
    );
    println!("  - Remove section asks first only when the Sequence uses it; Undo in the toast");
    println!("    (or Ctrl+Z on the section list) brings back the lyrics, slides and occurrences.");
    println!(
        "  - Leave through Back, the rail or Quit with unsaved text: it is saved first, and a"
    );
    println!("    failed save offers Retry save, Keep editing or Leave without saving.");
    println!("  - Close and reopen the lab parish: sections, slides and thumbnails are unchanged.");
    println!("Sequence path: with Verse 1 and Chorus written, open the Sequence tab:");
    println!("  - Enter on Add first occurrence, then F6 to Available sections: Add Chorus, Add");
    println!("    Verse 1, Add Chorus again. Each is placed after the selected row and selected.");
    println!("  - Repeat on the Chorus row adds another occurrence of the same section; edit the");
    println!("    Chorus in Lyrics and both occurrences show the new text, with no copied lyrics.");
    println!("  - Expand all shows each occurrence's slides; Collapse all changes nothing else.");
    println!("  - Focus a row, Alt+Down then Alt+Up: it moves with its slides and returns.");
    println!("  - Manual drag check (no automated pointer test): drag a row by its grip onto the");
    println!("    upper or lower half of another row and drop it; it moves once, with its slides.");
    println!("  - Focus the last row and press Delete: the section stays in Lyrics, the previous");
    println!("    row is selected and focused; Ctrl+Z brings it back selected and focused.");
    println!("  - Delete does nothing while a text field, a dialog or a row action has focus.");
    println!("  - Approve pagination: edit a section in Lyrics and the notice \"Pagination needs");
    println!("    review\" shows for that section only; Approve pagination clears it, and Ctrl+Z");
    println!("    returns it. Reordering the Sequence never changes any section's approval.");
    println!("  - The header readiness reads Needs review until every used section is approved.");
    println!("History path: edit an Arrangement, leave, reopen and edit again; open History:");
    println!("  - Sessions list newest first; open the older one and choose its revision. The");
    println!("    comparison has Overview, Lyrics, Sequence and Slides, marks what changed and");
    println!("    has no edit control. Restore this revision, then Restore entire Arrangement:");
    println!("    one later revision is written and the editor reopens on the old text.");
    println!("  - Undo in the toast writes another later revision; every earlier one stays.");
    println!("Archive path: Arrangement actions > Archive arrangement > Archive:");
    println!("  - You return to the Song; the toast offers Undo. Turn on Include archived,");
    println!("    Inspect the Arrangement (read-only), Restore from the banner, archive again.");
    println!("  - A Song archives the same way and keeps its Arrangements' own states.");
    println!("Delete path: archived Arrangement or Song actions > Delete permanently...:");
    println!("  - The dialog lists what still uses it, grouped, with no delete action; an");
    println!("    archived child Arrangement lists Inspect. A Mass plan reference cannot be made");
    println!("    here (no Mass plans yet), so the blocked path by a Mass plan is test-only;");
    println!("    the automated E2E plants one and removes it.");
    println!("  - With nothing using it, the dialog says what is erased and how many revisions");
    println!("    of History go with it. Delete permanently has no Undo; the Song and its");
    println!("    History stay when only an Arrangement is deleted.");

    let preferences = dir.join("preferences");
    application().run(move |cx: &mut App| {
        let runtime = match open_native_runtime_at(cx, &preferences) {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("the native runtime did not open: {error:?}");
                cx.quit();
                return;
            }
        };
        // As the application does: the slot outlives the window and stops the
        // storage worker when the app quits.
        let slot = StorageSlot::new();
        slot.shutdown_on_quit(cx);
        let result = runtime.operator().update(cx, |shell, window, cx| {
            let store = cx.new(|cx| LibraryStore::open(dir.clone(), slot.clone(), cx));
            shell.set_library_store(store, window, cx);
            shell.open(ordo::shell::rail::Destination::Library, window, cx);
        });
        if let Err(error) = result {
            eprintln!("the operator window closed before the lab could attach: {error}");
            cx.quit();
        }
        // The runtime value must live as long as the app does.
        std::mem::forget(runtime);
    });
    Ok(())
}
