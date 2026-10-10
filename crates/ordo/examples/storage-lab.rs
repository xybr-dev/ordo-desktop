//! Storage lab for the T0.7 manual review (WP3).
//!
//! `cargo run -p ordo --example storage-lab` opens a window over a
//! `storage::Storage` worker on a fresh temporary database. The path is printed
//! to stdout. The window shows the migrated schema, journal mode, the
//! foreign_keys flag, the parish name, the note count, and the last outcome.
//!
//! Buttons: "Add note" appends a note, "Rename parish" sets a numbered name,
//! "Refresh" re-reads the snapshot, and "Shutdown" consumes the handle and
//! shows the `ShutdownOutcome`. Later buttons report `WorkerUnavailable`,
//! because the handle has been consumed. Every storage future is awaited in a
//! `cx.spawn` task, so the GPUI thread never waits on SQLite. Closing the window
//! drops the entity and therefore the `Storage`, which requests shutdown without
//! joining.

use std::{
    io::Write as _,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use gpui::{
    App, AppContext as _, Bounds, ClickEvent, Context, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, WindowBounds,
    WindowOptions, div, px, rgb, size,
};
use gpui_platform::application;
use storage::{SchemaStatus, ShutdownOutcome, Storage, StorageConfig, StorageError};

/// Hex prefix length shown for each migration checksum.
const CHECKSUM_PREFIX: usize = 12;

struct StorageLab {
    db_path: PathBuf,
    /// `None` once Shutdown has consumed the handle.
    storage: Option<Storage>,
    schema: Option<SchemaStatus>,
    parish_name: Option<String>,
    note_count: Option<u64>,
    note_seq: u32,
    rename_seq: u32,
    refresh_seq: u32,
    last_outcome: String,
}

impl StorageLab {
    fn new(db_path: PathBuf) -> Self {
        let storage = Storage::spawn(StorageConfig::new(&db_path));
        Self {
            db_path,
            storage: Some(storage),
            schema: None,
            parish_name: None,
            note_count: None,
            note_seq: 0,
            rename_seq: 0,
            refresh_seq: 0,
            last_outcome: "opening database".to_owned(),
        }
    }

    /// Awaits the open sequence, then reads the first snapshot.
    fn start(&mut self, cx: &mut Context<Self>) {
        let Some(storage) = self.storage.as_ref() else {
            return;
        };
        let ready = storage.ready();
        cx.spawn(async move |this, cx| {
            let result = ready.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(status) => {
                        this.schema = Some(status);
                        this.last_outcome = "ready".to_owned();
                        this.refresh(cx);
                    }
                    Err(error) => this.last_outcome = format!("open refused: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Re-reads the schema status, parish name, and note count.
    /// The Refresh button: reports its own outcome, unlike the refresh that
    /// follows an operation, which keeps that operation's outcome visible.
    fn refresh_clicked(&mut self, cx: &mut Context<Self>) {
        self.refresh_seq += 1;
        self.last_outcome = format!("refreshed (#{})", self.refresh_seq);
        self.refresh(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(storage) = self.storage.as_ref() else {
            self.report_unavailable(cx);
            return;
        };
        let schema = storage.schema_status();
        let name = storage.parish_name();
        let count = storage.parish_note_count();
        cx.spawn(async move |this, cx| {
            let schema = schema.await;
            let name = name.await;
            let count = count.await;
            let _ = this.update(cx, |this, cx| {
                match schema {
                    Ok(status) => this.schema = Some(status),
                    Err(error) => this.last_outcome = format!("refresh: {error}"),
                }
                match name {
                    Ok(name) => this.parish_name = name,
                    Err(error) => this.last_outcome = format!("refresh: {error}"),
                }
                match count {
                    Ok(count) => this.note_count = Some(count),
                    Err(error) => this.last_outcome = format!("refresh: {error}"),
                }
                // A successful refresh keeps the previous outcome, so an
                // operation's error (e.g. a refused note) stays visible.
                cx.notify();
            });
        })
        .detach();
    }

    fn add_note(&mut self, cx: &mut Context<Self>) {
        let Some(storage) = self.storage.as_ref() else {
            self.report_unavailable(cx);
            return;
        };
        self.note_seq += 1;
        let body = format!("lab note {}", self.note_seq);
        let pending = storage.add_parish_note(body);
        cx.spawn(async move |this, cx| {
            let result = pending.await;
            let _ = this.update(cx, |this, cx| {
                this.last_outcome = match result {
                    Ok(id) => format!("added note row {id}"),
                    // Notes reference the parish profile (migration 0002), so
                    // foreign keys refuse a note until the parish is named.
                    Err(error @ StorageError::Sqlite(_)) => {
                        format!("add note refused: {error} (rename the parish first)")
                    }
                    Err(error) => format!("add note: {error}"),
                };
                this.refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn rename_parish(&mut self, cx: &mut Context<Self>) {
        let Some(storage) = self.storage.as_ref() else {
            self.report_unavailable(cx);
            return;
        };
        self.rename_seq += 1;
        let name = format!("St Mary's Parish {}", self.rename_seq);
        let pending = storage.set_parish_name(name);
        cx.spawn(async move |this, cx| {
            let result = pending.await;
            let _ = this.update(cx, |this, cx| {
                this.last_outcome = match result {
                    Ok(()) => "parish renamed".to_owned(),
                    Err(error) => format!("rename: {error}"),
                };
                this.refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Consumes the handle. Later operations find no handle and report
    /// `WorkerUnavailable`, the same error a dead worker returns.
    fn shutdown(&mut self, cx: &mut Context<Self>) {
        let Some(storage) = self.storage.take() else {
            self.report_unavailable(cx);
            return;
        };
        let pending = storage.shutdown();
        self.last_outcome = "shutting down".to_owned();
        cx.spawn(async move |this, cx| {
            let outcome = pending.await;
            let _ = this.update(cx, |this, cx| {
                this.last_outcome = format!("shutdown: {}", describe_shutdown(&outcome));
                cx.notify();
            });
        })
        .detach();
    }

    fn report_unavailable(&mut self, cx: &mut Context<Self>) {
        self.last_outcome = format!("{}", StorageError::WorkerUnavailable);
        cx.notify();
    }
}

fn describe_shutdown(outcome: &ShutdownOutcome) -> String {
    match outcome {
        ShutdownOutcome::Clean => "Clean".to_owned(),
        ShutdownOutcome::WorkerPanicked(message) => format!("WorkerPanicked({message})"),
        ShutdownOutcome::AlreadyStopped => "AlreadyStopped".to_owned(),
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(0x2d4a6b))
        .text_color(rgb(0xffffff))
        .cursor_pointer()
        .child(label)
        .on_click(on_click)
}

fn line(text: String) -> impl IntoElement {
    div().child(text)
}

impl Render for StorageLab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div()
            .size_full()
            .p_4()
            .gap_2()
            .flex()
            .flex_col()
            .bg(rgb(0x1b1f24))
            .text_color(rgb(0xe6e6e6))
            .child(line(format!("database: {}", self.db_path.display())));

        match &self.schema {
            Some(status) => {
                panel = panel
                    .child(line(format!("schema version: {}", status.version)))
                    .child(line(format!("journal mode: {}", status.journal_mode)))
                    .child(line(format!("foreign_keys: {}", status.foreign_keys)));
                for migration in &status.applied {
                    let checksum: String =
                        migration.checksum.chars().take(CHECKSUM_PREFIX).collect();
                    panel = panel.child(line(format!(
                        "  migration {} {} {}",
                        migration.id, migration.name, checksum
                    )));
                }
            }
            None => panel = panel.child(line("schema: not available".to_owned())),
        }

        let name = match &self.parish_name {
            Some(name) => name.clone(),
            None => "(not set)".to_owned(),
        };
        let count = self
            .note_count
            .map_or_else(|| "(unknown)".to_owned(), |count| count.to_string());
        panel
            .child(line(format!("parish name: {name}")))
            .child(line(format!("note count: {count}")))
            .child(line(format!("last outcome: {}", self.last_outcome)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button(
                        "add-note",
                        "Add note",
                        cx.listener(|this, _: &ClickEvent, _, cx| this.add_note(cx)),
                    ))
                    .child(button(
                        "rename-parish",
                        "Rename parish",
                        cx.listener(|this, _: &ClickEvent, _, cx| this.rename_parish(cx)),
                    ))
                    .child(button(
                        "refresh",
                        "Refresh",
                        cx.listener(|this, _: &ClickEvent, _, cx| this.refresh_clicked(cx)),
                    ))
                    .child(button(
                        "shutdown",
                        "Shutdown",
                        cx.listener(|this, _: &ClickEvent, _, cx| this.shutdown(cx)),
                    )),
            )
    }
}

/// A fresh directory under the system temp dir. It is never the Parish data
/// directory, and it is left in place so the database can be inspected.
fn fresh_db_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("ordo-storage-lab-{}-{nanos}", std::process::id()));
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {error}", dir.display());
        std::process::exit(1);
    }
    dir.join("parish.sqlite3")
}

fn main() {
    let db_path = fresh_db_path();
    println!("{}", db_path.display());
    let _ = std::io::stdout().flush();

    application().run(move |cx: &mut App| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(860.0), px(560.0)),
                cx,
            ))),
            ..Default::default()
        };
        let opened = cx.open_window(options, move |_, cx| {
            cx.new(|cx| {
                let mut lab = StorageLab::new(db_path);
                lab.start(cx);
                lab
            })
        });
        if let Err(error) = opened {
            eprintln!("cannot open the storage lab window: {error:?}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
