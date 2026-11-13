//! Owner lab for the T2.2 edit session over real storage.
//!
//! `cargo run -p ordo --example edit-session-lab` opens a window with the
//! reference editor (title, notes, a reorderable list) over one seeded
//! Arrangement in a fresh Parish at `%TEMP%\ordo-edit-session-lab-<pid>`. The
//! side panel lists the Arrangement's revisions (kind, History session, time)
//! and refreshes on every save-state change.
//!
//! Try: type in a field and wait one second; reorder rows; click outside the
//! fields and press Ctrl+Z / Ctrl+Y (editor Undo and Redo, each an Edit
//! revision); press "Fail next save", edit, and see "Couldn't save", then
//! "Retry save". Closing the window flushes pending text first (the session's
//! navigation guard); if the flush fails the window stays open.
//!
//! The database path is printed at start. The folder is left in place.

#[path = "../tests/support/storage_port.rs"]
mod storage_port;

use std::{
    cell::Cell,
    fs,
    future::Future,
    pin::pin,
    rc::Rc,
    sync::Arc,
    task::{Context as TaskContext, Poll, Wake, Waker},
    thread::{self, Thread},
};

use domain::{identity::ArrangementId, revisions::RevisionEntity};
use gpui::{
    App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Window, WindowBounds, WindowOptions, div, px, rgb, size,
};
use gpui_platform::application;
use ordo::edit_session::{
    EditSession, EditSessionEvent, NavigationGuard, RevisionPort, SaveState,
    harness::{EditHarness, HarnessContent},
};
use storage::{NewArrangement, NewSection, NewSong, Storage, StorageConfig};
use storage_port::{SharedStorage, StoragePort};
use ui::{
    environment::{ResolvedAppearance, UiEnvironment, UiPreferences},
    primitives::button::Button,
    tokens,
};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Parks the thread until `future` completes. Only used before the window
/// opens, to seed the Parish; the GPUI thread never calls it.
fn block_on<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = TaskContext::from_waker(&waker);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        thread::park();
    }
}

/// One row of the revision panel.
struct Row {
    kind: String,
    session: String,
    time: String,
}

/// The editor beside its revision list.
struct Lab {
    harness: Entity<EditHarness>,
    session: Entity<EditSession<HarnessContent>>,
    storage: SharedStorage,
    arrangement: ArrangementId,
    fail_next: Rc<Cell<bool>>,
    rows: Vec<Row>,
    environment: UiEnvironment,
    _subscription: Subscription,
}

impl Lab {
    fn new(
        storage: SharedStorage,
        arrangement: ArrangementId,
        session: Entity<EditSession<HarnessContent>>,
        fail_next: Rc<Cell<bool>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let environment = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
        let harness = cx.new(|cx| EditHarness::new(session.clone(), environment, window, cx));
        let subscription = cx.subscribe(&session, |this, _, event, cx| {
            if matches!(event, EditSessionEvent::StateChanged) {
                this.refresh(cx);
            }
        });
        let lab = Self {
            harness,
            session,
            storage,
            arrangement,
            fail_next,
            rows: Vec::new(),
            environment,
            _subscription: subscription,
        };
        lab.refresh(cx);
        lab
    }

    fn refresh(&self, cx: &mut Context<Self>) {
        let arrangement = self.arrangement;
        let list = self
            .storage
            .with(|s| s.revisions(RevisionEntity::Arrangement(arrangement)));
        cx.spawn(async move |this, cx| {
            let Ok(records) = list.await else { return };
            let rows = records
                .iter()
                .map(|record| Row {
                    kind: format!("{:?}", record.kind),
                    session: record.session.map_or_else(
                        || "-".to_owned(),
                        |session| session.entity().to_string().chars().take(8).collect(),
                    ),
                    time: record.created_at.to_string(),
                })
                .collect();
            this.update(cx, |this, cx| {
                this.rows = rows;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Lab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let environment = self.environment;
        let fail_next = self.fail_next.clone();
        let this = cx.entity();
        let session = self.session.clone();
        let failing = self.fail_next.get();
        let panel = div()
            .w(px(380.0))
            .flex()
            .flex_col()
            .gap_1()
            .p_4()
            .child(format!("Revisions ({})", self.rows.len()))
            .children(self.rows.iter().enumerate().map(|(index, row)| {
                SharedString::from(format!(
                    "{index:>2}  {:<11} {}  {}",
                    row.kind, row.session, row.time
                ))
            }));
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(self.harness.clone())
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .p_4()
                            .child(
                                Button::new(
                                    "lab-fail-next",
                                    if failing {
                                        "Fail next save: on"
                                    } else {
                                        "Fail next save"
                                    },
                                    environment,
                                )
                                .on_activate(move |_, cx| {
                                    fail_next.set(!fail_next.get());
                                    this.update(cx, |_, cx| cx.notify());
                                }),
                            )
                            .child(
                                Button::new("lab-retry", "Retry save", environment).on_activate(
                                    move |_, cx| {
                                        session.update(cx, |session, cx| {
                                            if session.save_state() == SaveState::CouldntSave {
                                                session.retry_save(cx);
                                            }
                                        });
                                    },
                                ),
                            ),
                    ),
            )
            .child(panel)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("ordo-edit-session-lab-{}", std::process::id()));
    if root.exists() {
        return Err(format!("{} already exists; remove it and rerun", root.display()).into());
    }
    fs::create_dir_all(&root)?;
    let config = StorageConfig::in_directory(&root);
    println!("database: {}", config.path.display());

    let storage = Storage::spawn(config);
    block_on(storage.ready())?;
    let song = block_on(storage.create_song(NewSong {
        title: "Kyrie".to_owned(),
        aliases: Vec::new(),
        original_attribution: None,
        provenance: None,
        rights_notes: None,
        liturgical_moments: Vec::new(),
        occasions: Vec::new(),
    }))?;
    let arrangement = block_on(
        storage.create_arrangement(NewArrangement {
            song: song.id,
            title: Some("SATB".to_owned()),
            notes: None,
            languages: vec!["en".to_owned()],
            choirs: Vec::new(),
            rendition_attribution: None,
            occasions: Vec::new(),
            sections: ["Verse", "Refrain", "Coda"]
                .map(|label| NewSection {
                    label: Some(label.to_owned()),
                    lyrics: label.to_owned(),
                })
                .to_vec(),
            sequence: vec![0, 1, 2],
        }),
    )?
    .id;

    let storage = SharedStorage::new(storage);
    let (port, content) = block_on(StoragePort::open(storage.clone(), arrangement))?;
    let fail_next = port.fail_next.clone();
    let port: Rc<dyn RevisionPort<HarnessContent>> = Rc::new(port);

    application().run(move |cx: &mut App| {
        ui::init(cx).expect("the UI environment should initialize");
        ordo::edit_session::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let session = cx.new(|cx| {
                    EditSession::new(RevisionEntity::Arrangement(arrangement), content, port, cx)
                });
                guard_close(&session, window, cx);
                cx.new(|cx| Lab::new(storage, arrangement, session, fail_next, window, cx))
            },
        );
        opened.expect("the edit session lab window should open");
        cx.activate(true);
    });
    println!("database: {}", root.join("parish.sqlite3").display());
    Ok(())
}

/// Wires the navigation guard named in the `edit_session` module docs: the
/// window close and the app quit both flush first. A failed flush keeps the
/// window open so the operator can Retry save.
fn guard_close(session: &Entity<EditSession<HarnessContent>>, window: &mut Window, cx: &mut App) {
    let cleared = Rc::new(Cell::new(false));
    let for_close = session.clone();
    let for_quit = session.clone();
    window.on_window_should_close(cx, move |window, cx| {
        if cleared.get() {
            return true;
        }
        let task = for_close.update(cx, |session, cx| session.request_navigation(cx));
        let handle = window.window_handle();
        let cleared = cleared.clone();
        let session = for_close.clone();
        cx.spawn(async move |cx| {
            if task.await == NavigationGuard::Proceed {
                cleared.set(true);
                cx.update(|cx| {
                    session.update(cx, |session, cx| session.leave(cx));
                    handle
                        .update(cx, |_, window, _| window.remove_window())
                        .ok();
                });
            }
        })
        .detach();
        false
    });
    cx.on_app_quit(move |cx| {
        let task = for_quit.update(cx, |session, cx| session.request_navigation(cx));
        async move {
            task.await;
        }
    })
    .detach();
}
