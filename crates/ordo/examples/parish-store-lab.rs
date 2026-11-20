//! Owner lab for the T2.1 parish store, with no window.
//!
//! `cargo run -p ordo --example parish-store-lab` creates a Parish, a Choir, a
//! Song, an Arrangement and a Theme with a PNG background through the storage
//! Module Interface, in a fresh directory `ordo-parish-store-lab-<pid>` under
//! the system temporary directory. It restarts the storage worker, reads every
//! record back and checks it against what was created, then prints the
//! database path, the assets directory listing and the `verify_assets` report.
//!
//! It then shows a refused write. A Theme whose background has an unsupported
//! media type (`image/gif`) fails with `InvalidInput` before any byte is hashed
//! or written, so the assets directory does not change. The lab uses this
//! normal-API refusal on purpose: the injected asset-write failure exists only
//! under the storage `fault-injection` feature, which a normal build does not
//! enable.
//!
//! The directory is left in place. The last line names the database to open
//! with `sqlite3`. Each run uses a directory named by its process id, and an
//! existing directory is refused rather than overwritten.

use std::{
    fs,
    future::Future,
    path::Path,
    pin::pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread::{self, Thread},
};

use storage::{
    AssetUpload, NewArrangement, NewChoir, NewParish, NewSection, NewSong, NewTheme,
    ShutdownOutcome, Storage, StorageConfig, StorageError,
};

/// Unparks the thread that is blocked in [`block_on`].
struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Parks the calling thread until `future` completes. The lab has no GPUI
/// thread to protect, and the workspace has no executor dependency to reuse.
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

/// Prints the sorted entry names directly under `assets/`.
fn print_assets(assets: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut names = Vec::new();
    for entry in fs::read_dir(assets)? {
        names.push(entry?.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    for name in &names {
        println!("  assets/{name}");
    }
    Ok(names)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("ordo-parish-store-lab-{}", std::process::id()));
    if root.exists() {
        return Err(format!("{} already exists; remove it and rerun", root.display()).into());
    }
    fs::create_dir_all(&root)?;
    let config = StorageConfig::in_directory(&root);
    let db = config.path.clone();
    let assets = config.assets_dir.clone();

    // 1. Open the store on the fresh directory.
    let storage = Storage::spawn(config.clone());
    let status = block_on(storage.ready())?;
    println!(
        "schema version {}, {} migrations applied, journal {}",
        status.version,
        status.applied.len(),
        status.journal_mode
    );

    // 2. Create the Parish, Choir, Song, Arrangement and a Theme with a background.
    let parish = block_on(storage.create_parish(NewParish {
        name: "St Mary".to_owned(),
    }))?;
    let choir = block_on(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
        short_label: None,
        languages: Vec::new(),
        notes: None,
    }))?;
    let song = block_on(storage.create_song(NewSong {
        title: "Kyrie".to_owned(),
        aliases: vec!["Lord have mercy".to_owned()],
        original_attribution: Some("Anon".to_owned()),
        provenance: None,
        rights_notes: None,
        liturgical_moments: vec!["Penitential Act".to_owned()],
        occasions: vec!["Lent".to_owned()],
    }))?;
    let arrangement = block_on(storage.create_arrangement(NewArrangement {
        song: song.id,
        title: Some("SATB".to_owned()),
        notes: None,
        languages: vec!["en".to_owned()],
        choirs: vec![choir.id],
        rendition_attribution: Some("Arr. Smith".to_owned()),
        occasions: vec!["Lent".to_owned()],
        sections: vec![
            NewSection {
                label: Some("Verse".to_owned()),
                lyrics: "Line one\nLine two".to_owned(),
            },
            NewSection {
                label: Some("Refrain".to_owned()),
                lyrics: "Refrain text".to_owned(),
            },
        ],
        sequence: vec![0, 1, 0],
    }))?;
    let background = b"\x89PNG\r\n\x1a\nlab background".to_vec();
    let theme = block_on(storage.create_theme(NewTheme {
        name: "Easter".to_owned(),
        background: Some(AssetUpload {
            bytes: background.clone(),
            media_type: "image/png".to_owned(),
        }),
    }))?;
    let asset = theme.background.clone().ok_or("theme has no background")?;

    // 3. Restart: shut down cleanly, then spawn a new worker on the same directory.
    let outcome = block_on(storage.shutdown());
    println!("shutdown before restart: {outcome:?}");
    if outcome != ShutdownOutcome::Clean {
        return Err("the first worker did not shut down cleanly".into());
    }
    let storage = Storage::spawn(config);
    block_on(storage.ready())?;

    // 4. Read every record back and compare it with what was created.
    let rehydrated = block_on(storage.parish())? == Some(parish.clone())
        && block_on(storage.choirs())? == vec![choir.clone()]
        && block_on(storage.song(song.id))? == song
        && block_on(storage.arrangement(arrangement.id))? == arrangement
        && block_on(storage.theme(theme.id))? == theme
        && block_on(storage.read_asset(asset.id))? == background;
    if !rehydrated {
        return Err("records did not rehydrate exactly after restart".into());
    }
    println!("rehydrated after restart: exact match");
    println!("parish:      {parish:?}");
    println!("choir:       {choir:?}");
    println!("song:        {song:?}");
    println!("arrangement: {arrangement:?}");
    println!("theme:       {theme:?}");
    println!("asset bytes: {} read back and verified", background.len());

    println!("database: {}", db.display());
    println!("assets directory:");
    let before_refusal = print_assets(&assets)?;
    println!("verify_assets: {:?}", block_on(storage.verify_assets())?);

    // 5. A refused write: an unsupported media type writes nothing.
    let refused = block_on(storage.create_theme(NewTheme {
        name: "Refused".to_owned(),
        background: Some(AssetUpload {
            bytes: b"GIF89a lab".to_vec(),
            media_type: "image/gif".to_owned(),
        }),
    }));
    match refused {
        Err(error @ StorageError::InvalidInput { .. }) => {
            println!("refused write: {error} ({error:?})");
        }
        other => return Err(format!("expected InvalidInput, got {other:?}").into()),
    }
    let after_refusal = print_assets(&assets)?;
    if after_refusal != before_refusal {
        return Err("the refused write changed the assets directory".into());
    }
    println!("assets directory unchanged by the refused write");

    let outcome = block_on(storage.shutdown());
    println!("shutdown: {outcome:?}");
    println!("Inspect with: sqlite3 {}", db.display());
    Ok(())
}
