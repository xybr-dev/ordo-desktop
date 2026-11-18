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

use domain::{
    identity::{ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId, SongId, Timestamp},
    revisions::RevisionKind,
    song_library::{
        ArrangementDraft, OccurrenceDraft, PaginationState, SectionDraft, SongDraft, SongLibrary,
        import::{
            Choices, ImportArrangement, ImportDocument, ImportSection, Outcome, SequenceEntry,
            SongChoice, Staged, StagedDocument, plan, review_with_choices,
        },
    },
};
use gpui::{App, AppContext as _};
use gpui_platform::application;
use ordo::{
    open_native_runtime_at,
    songs::{LibraryStore, StorageSlot},
};
use storage::{
    ArrangementContent, ImportBatch, ImportedArrangement, NewArrangement, NewChoir, NewSection,
    NewSong, OccurrenceContent, SectionContent, ShutdownOutcome, Storage, StorageConfig,
    exchange::{
        InspectLimits, ManifestOverrides, SongImportRead, pack_for_tests, read_song_import_bytes,
    },
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
    seed_baseline(&storage)?;
    match block_on(storage.shutdown()) {
        ShutdownOutcome::Clean => Ok(()),
        other => Err(format!("the seeding store did not stop cleanly: {other:?}").into()),
    }
}

// ---- Import: the batch, its baseline library and the verdict timing ----

/// The five documents of tests/fixtures/song-import/batch, by file name.
const BATCH: [(&str, &[u8]); 5] = [
    (
        "01-new-song.ordo",
        include_bytes!("../../../tests/fixtures/song-import/batch/01-new-song.json"),
    ),
    (
        "02-unchanged.ordo",
        include_bytes!("../../../tests/fixtures/song-import/batch/02-unchanged.json"),
    ),
    (
        "03-attachable.ordo",
        include_bytes!("../../../tests/fixtures/song-import/batch/03-attachable.json"),
    ),
    (
        "04-ambiguous-replacement.ordo",
        include_bytes!("../../../tests/fixtures/song-import/batch/04-ambiguous-replacement.json"),
    ),
    (
        "05-invalid-tampered-hash.ordo",
        include_bytes!("../../../tests/fixtures/song-import/batch/05-invalid-tampered-hash.json"),
    ),
];

/// A Markdown song with a forward `[Repeat: Koro]` that resolves and a
/// `[Repeat: Korro]` that names no section.
const FORWARD_REPEAT: &str =
    include_str!("../../../tests/fixtures/song-import/markdown/forward-repeat-unresolved.md");

/// An id for the nth thing, distinct from every other n.
fn entity(n: u64) -> EntityId {
    let mut bytes = [0xab; 16];
    bytes[8..].copy_from_slice(&n.to_be_bytes());
    EntityId::from_random_bytes(bytes)
}

/// A fixture payload read back as the typed document it describes.
fn decoded(payload: &[u8]) -> Result<ImportDocument, Failure> {
    let bytes = pack_for_tests(&ManifestOverrides::default(), payload, &[]);
    match read_song_import_bytes(&bytes, &InspectLimits::default()) {
        SongImportRead::Valid(document) => Ok(document),
        other => Err(format!("a fixture should be a valid document: {other:?}").into()),
    }
}

/// What tests/fixtures/song-import/batch/seed.json describes: Salamat, Ginoo
/// and Bread of Life as it was before the batch reworded it.
fn baseline() -> Result<Vec<ImportDocument>, Failure> {
    let unchanged = decoded(BATCH[1].1)?;
    let mut bread = decoded(BATCH[3].1)?;
    bread.warnings.clear();
    let arrangement = &mut bread.arrangements[0];
    arrangement.warnings.clear();
    arrangement.sections.truncate(2);
    arrangement.sections[1].lyrics = "You gave us your body\na gift for the hungry".to_owned();
    arrangement.sequence.truncate(3);
    Ok(vec![unchanged, bread])
}

/// Writes the baseline into the lab parish under the fixtures' own identities,
/// through the storage Interface, so the batch classifies as in the tests.
fn seed_baseline(storage: &Storage) -> Result<(), Failure> {
    let documents = baseline()?;
    let mut batch = ImportBatch {
        choirs: Vec::new(),
        songs: Vec::new(),
        arrangements: Vec::new(),
        replacements: Vec::new(),
        first_kind: RevisionKind::Import,
    };
    let mut next = 1_000u64;
    let mut mint = || {
        next += 1;
        entity(next)
    };
    for document in documents {
        batch.songs.push((
            document.id,
            NewSong {
                title: document.song.title.clone(),
                aliases: document.song.aliases.clone(),
                original_attribution: document.song.original_attribution.clone(),
                provenance: document.song.provenance.clone(),
                rights_notes: document.song.rights_notes.clone(),
                liturgical_moments: document.song.liturgical_moments.clone(),
                occasions: document.song.occasions.clone(),
            },
        ));
        for arrangement in document.arrangements {
            let mut choirs = Vec::new();
            for name in &arrangement.choirs {
                let known = batch.choirs.iter().find(|(_, choir)| &choir.name == name);
                let id = match known {
                    Some((id, _)) => *id,
                    None => {
                        let id = ChoirId::from_entity(mint());
                        batch.choirs.push((
                            id,
                            NewChoir {
                                name: name.clone(),
                                short_label: None,
                                languages: Vec::new(),
                                notes: None,
                            },
                        ));
                        id
                    }
                };
                choirs.push(id);
            }
            choirs.sort();
            let sections: Vec<SectionContent> = arrangement
                .sections
                .iter()
                .map(|section| SectionContent {
                    id: SectionId::from_entity(mint()),
                    label: section.label.clone(),
                    lyrics: section.lyrics.clone(),
                    pagination: PaginationState::NeedsReview,
                })
                .collect();
            let sequence = arrangement
                .sequence
                .iter()
                .filter_map(|entry| match entry {
                    SequenceEntry::Section(at) => sections.get(*at).map(|section| section.id),
                    SequenceEntry::Unresolved { .. } => None,
                })
                .map(|section| OccurrenceContent {
                    id: OccurrenceId::from_entity(mint()),
                    section,
                })
                .collect();
            batch.arrangements.push(ImportedArrangement {
                id: arrangement.id,
                song: document.id,
                content: ArrangementContent {
                    title: arrangement.title,
                    languages: arrangement.languages,
                    choirs,
                    rendition_attribution: arrangement.rendition_attribution,
                    occasions: arrangement.occasions,
                    notes: arrangement.notes,
                    sections,
                    sequence,
                },
            });
        }
    }
    block_on(storage.commit_import(batch))?;
    Ok(())
}

/// Writes the five documents and the Markdown song the Import path uses.
fn write_batch(dir: &Path) -> Result<std::path::PathBuf, Failure> {
    let folder = dir.join("import-batch");
    std::fs::create_dir_all(&folder)?;
    for (name, payload) in BATCH {
        let bytes = pack_for_tests(&ManifestOverrides::default(), payload, &[]);
        std::fs::write(folder.join(name), bytes)?;
    }
    std::fs::write(
        folder.join("forward-repeat-unresolved.md"),
        FORWARD_REPEAT.as_bytes(),
    )?;
    Ok(folder)
}

/// The generated document of Song number n; variant 1 rewords its first
/// verse.
fn generated(n: u64, variant: u8) -> ImportDocument {
    let verse = if variant == 0 {
        format!("Awit {n}, unang linya\nawit {n}, ikaduhang linya")
    } else {
        format!("Awit {n}, gibag-o nga linya\nawit {n}, ikaduhang linya")
    };
    ImportDocument {
        id: SongId::from_entity(entity(10_000 + n)),
        song: SongDraft {
            aliases: vec![format!("Song {n}")],
            original_attribution: Some(format!("Composer {}", n % 40)),
            ..SongDraft::titled(format!("Awit nga Numero {n}"))
        },
        arrangements: vec![ImportArrangement {
            id: ArrangementId::from_entity(entity(20_000 + n)),
            title: None,
            languages: vec!["Cebuano".to_owned()],
            choirs: Vec::new(),
            rendition_attribution: None,
            occasions: Vec::new(),
            notes: None,
            sections: vec![
                ImportSection {
                    label: Some("Verse 1".to_owned()),
                    lyrics: verse,
                },
                ImportSection {
                    label: Some("Chorus".to_owned()),
                    lyrics: format!("Aleluya {n}\nsalamat sa Ginoo"),
                },
            ],
            sequence: vec![
                SequenceEntry::Section(0),
                SequenceEntry::Section(1),
                SequenceEntry::Section(0),
            ],
            warnings: Vec::new(),
        }],
        warnings: Vec::new(),
    }
}

/// The library of Songs numbered 0 up to count, as the generator wrote them.
fn generated_library(count: u64) -> Result<SongLibrary, Failure> {
    let mut library = SongLibrary::new();
    let at = Timestamp::from_millis(0);
    let mut next = 100_000u64;
    for n in 0..count {
        let document = generated(n, 0);
        library.create_song(document.id, at, document.song.clone())?;
        for arrangement in &document.arrangements {
            let sections: Vec<SectionDraft> = arrangement
                .sections
                .iter()
                .map(|section| {
                    next += 1;
                    SectionDraft {
                        id: SectionId::from_entity(entity(next)),
                        label: section.label.clone(),
                        lyrics: section.lyrics.clone(),
                        pagination: PaginationState::NeedsReview,
                    }
                })
                .collect();
            let sequence = arrangement
                .sequence
                .iter()
                .filter_map(|entry| match entry {
                    SequenceEntry::Section(at) => sections.get(*at).map(|s| s.id),
                    SequenceEntry::Unresolved { .. } => None,
                })
                .map(|section| {
                    next += 1;
                    OccurrenceDraft::new(OccurrenceId::from_entity(entity(next)), section)
                })
                .collect();
            library.create_arrangement(
                arrangement.id,
                document.id,
                at,
                ArrangementDraft {
                    languages: arrangement.languages.clone(),
                    sections,
                    sequence,
                    ..ArrangementDraft::default()
                },
                Vec::new(),
            )?;
        }
    }
    Ok(library)
}

/// The milliseconds one call took, best of several.
fn best_of<T>(runs: usize, mut call: impl FnMut() -> T) -> (f64, T) {
    let mut best = f64::MAX;
    let mut last = None;
    for _ in 0..runs {
        let started = std::time::Instant::now();
        let value = call();
        best = best.min(started.elapsed().as_secs_f64() * 1000.0);
        last = Some(value);
    }
    (best, last.expect("at least one run"))
}

/// Times what the GPUI thread does for one Import approved press and for one
/// Markdown recompute: the review a commit compares against, the plan, the
/// whole-library check, and the Markdown draft's possible-existing-Songs
/// ranking, for 50 documents against about 1,000 Songs. Run it in release.
fn time_import() -> Result<(), Failure> {
    const SONGS: u64 = 1_000;
    const DOCUMENTS: u64 = 50;
    let library = generated_library(SONGS)?;
    // 20 new Songs, 15 unchanged, 15 reworded (each decided as an Update).
    let mut documents = Vec::new();
    let mut choices = Choices::default();
    for at in 0..DOCUMENTS {
        let document = match at {
            0..=19 => generated(SONGS + at, 0),
            20..=34 => generated(at, 0),
            _ => generated(at, 1),
        };
        if at >= 35 {
            choices
                .arrangements
                .insert((at as usize, 0), Outcome::UpdateExisting);
        }
        documents.push(StagedDocument {
            source: format!("{at:02}.ordo"),
            content: Staged::Valid(document),
        });
    }
    let songs = std::collections::BTreeMap::<usize, SongChoice>::new();
    let (review_ms, reviews) = best_of(20, || review_with_choices(&library, &documents, &songs));
    let mut next = 1_000_000u64;
    let (plan_ms, planned) = best_of(20, || {
        let mut mint = || {
            next += 1;
            entity(next)
        };
        plan(&library, &documents, &reviews, &choices, &mut mint)
    });
    let planned = planned?;
    let (check_ms, checked) = best_of(20, || library.check_import(&planned));
    checked?;
    let total = review_ms + plan_ms + check_ms;
    println!(
        "import verdict, {DOCUMENTS} documents against {} Songs ({} new Songs, {} replacements planned):",
        library.songs().count(),
        planned.songs.len(),
        planned.replacements.len()
    );
    println!("  review_with_choices  {review_ms:8.3} ms");
    println!("  plan                 {plan_ms:8.3} ms");
    println!("  check_import         {check_ms:8.3} ms");
    println!("  verdict total        {total:8.3} ms   (budget 16 ms)");
    println!("  The store runs this verdict in a background task on a snapshot of the library;");
    println!("  the GPUI thread only pays for the snapshot clone below.");
    let (clone_ms, _) = best_of(20, || library.clone());
    println!("  library snapshot clone {clone_ms:8.3} ms   (on the GPUI thread)");
    // The Markdown draft ranks its title against the whole library on every
    // recompute. Matching is by identity key, so a title that is only a prefix
    // of many titles finds none of them; the cost is the scan over every Song
    // either way, and each label says what the case actually finds.
    for (name, title, aliases) in [
        ("no resemblance", "Zzzz unrelated title", vec![]),
        ("one exact title", "Awit nga Numero 500", vec![]),
        ("a prefix of many titles", "Awit nga Numero", vec![]),
        (
            "one alias match",
            "Zzzz unrelated title",
            vec!["Song 500".to_owned()],
        ),
    ] {
        let draft = SongDraft {
            aliases,
            original_attribution: Some("Composer 20".to_owned()),
            ..SongDraft::titled(title)
        };
        let (ms, found) = best_of(20, || library.possible_existing_songs(&draft));
        println!(
            "  possible_existing_songs ({name}, {} found) {ms:8.3} ms",
            found.len()
        );
    }
    Ok(())
}

fn main() -> Result<(), Failure> {
    if std::env::args().any(|argument| argument == "--time-import") {
        return time_import();
    }
    let dir = std::env::temp_dir().join(format!("ordo-songs-library-lab-{}", std::process::id()));
    if dir.exists() {
        return Err(format!("{} already exists; remove it and rerun", dir.display()).into());
    }
    std::fs::create_dir_all(&dir)?;
    seed(&dir)?;
    let batch = write_batch(&dir)?;
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

    println!("Import path: Library > Songs > Import .ordo, choose every .ordo file in");
    println!("  {}", batch.display());
    println!("  - The review lists five documents: Add new Song, Skip unchanged, Attach");
    println!("    Arrangement, Needs decision (Bread of Life) and Invalid (tampered hash).");
    println!("  - Import approved is off while a decision is open or until something can be");
    println!("    written; its reason is shown. Nothing is written until you press it.");
    println!("  - On Bread of Life, Compare contents (no edit control); choose Update existing");
    println!("    Arrangement and read its disclosure; Import approved then writes one batch.");
    println!(
        "  - The footer summary counts Songs, Arrangements, updates, Choirs, skipped, invalid."
    );
    println!("  - Open the updated Arrangement's History: its row reads Replaced from an import.");
    println!("  - Import the same folder again: every valid document is Skip unchanged and");
    println!("    Import approved stays off with nothing to import.");
    println!("Markdown path: Library > Songs > Import .ordo, choose forward-repeat-unresolved.md:");
    println!("  - The draft resolves the forward [Repeat: Koro] and flags the misspelled");
    println!(
        "    [Repeat: Korro]; correct it to Koro or leave it, then Save: an unresolved repeat"
    );
    println!("    saves as an empty placeholder");
    println!("    section; possible existing Songs show under the title as in New song.");
    println!(
        "Timing check: cargo run --release -p ordo --example songs-library-lab -- --time-import"
    );

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
