//! Development-only Live Control lab (T1.4 WP5).
//!
//! ```text
//! cargo run -p ordo --example live-control-lab -- [--size 1366x768] [--scale 200] [snapshot-dir]
//! ```
//!
//! Opens the Presenter view over a real Presentation Engine and a verified
//! snapshot (default: `tests/fixtures/presentations/normal-mass`), so the
//! header, rail placeholder, timeline placeholder and Output Dock can be
//! judged at any window size and Interface scale. It is not a workflow:
//! Presenter Mode entry is T1.6, and the product key bindings are T1.5.
//!
//! Lab-only keys (focus the window first): Left/Right previous/next Cue,
//! Space take Preview live, F freeze, B blackout, E enable output, D simulate
//! display lost, R simulate the recognized display returning.
//!
//! Safety and ownership:
//!
//! * The snapshot is read on the background executor; the window says
//!   "Loading..." until it is ready, or shows the load error.
//! * The bundled faces are verified when the window opens, and the lab fails
//!   closed with a visible message if they do not match.
//! * Preferences (the Grid/List choice) are read from and written under
//!   `target/live-control-lab/`, never `%APPDATA%`. Saves go to one worker
//!   thread, owned by `main`, which drops the sender when the window closes
//!   and joins the thread after the app loop ends.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::Instant,
};

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyBinding, ParentElement as _, Render, SharedString, Styled, Task,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, px, rgb, size,
};
use gpui_platform::application;
use ordo::{
    performance::display_path::verify_bundled_faces,
    presenter::{PresenterView, SavePreferences},
};
use platform::preferences::PreferencesFile;
use presentation::{
    commands::Command,
    snapshot_reader::{DirectorySource, LoadedSnapshot, SnapshotLoadError, read_snapshot},
    state::Millis,
};
use ui::{
    environment::{
        EnvironmentChange, InterfaceScale, ResolvedAppearance, UiEnvironment, UiPreferences,
    },
    tokens,
};

actions!(
    live_control_lab,
    [
        PreviousCue,
        NextCue,
        TakeLive,
        ToggleFreeze,
        ToggleBlackout,
        EnableOutput,
        SimulateDisplayLost,
        SimulateDisplayReturned,
    ]
);

const KEY_HELP: &str = "Left/Right Cue · Space take live · F freeze · B blackout · E enable output · D display lost · R display returned";

/// What the command line asked for.
#[derive(Debug, PartialEq)]
struct Args {
    width: f32,
    height: f32,
    scale: InterfaceScale,
    snapshot: PathBuf,
}

fn default_snapshot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/presentations/normal-mass")
}

fn parse_scale(text: &str) -> Option<InterfaceScale> {
    match text {
        "100" => Some(InterfaceScale::Percent100),
        "125" => Some(InterfaceScale::Percent125),
        "150" => Some(InterfaceScale::Percent150),
        "175" => Some(InterfaceScale::Percent175),
        "200" => Some(InterfaceScale::Percent200),
        _ => None,
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        width: 1366.0,
        height: 768.0,
        scale: InterfaceScale::Percent100,
        snapshot: default_snapshot(),
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--size" => {
                let value = args.next().ok_or("--size needs a value such as 1366x768")?;
                let (width, height) = value.split_once('x').ok_or("--size looks like 1366x768")?;
                parsed.width = width.parse().map_err(|_| "--size width is not a number")?;
                parsed.height = height
                    .parse()
                    .map_err(|_| "--size height is not a number")?;
                if parsed.width < 320.0 || parsed.height < 240.0 {
                    return Err("--size is too small to show the Presenter".into());
                }
            }
            "--scale" => {
                let value = args
                    .next()
                    .ok_or("--scale needs 100, 125, 150, 175 or 200")?;
                parsed.scale = parse_scale(&value).ok_or("--scale is 100, 125, 150, 175 or 200")?;
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => parsed.snapshot = PathBuf::from(arg),
        }
    }
    Ok(parsed)
}

fn lab_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/live-control-lab")
}

enum Stage {
    Loading,
    Failed(SharedString),
    Ready(Entity<PresenterView>),
}

/// The lab's root: shows the load stage, then hosts the Presenter and turns
/// the lab-only keys into commands. Owns the load task.
struct LabRoot {
    stage: Stage,
    focus: FocusHandle,
    environment: UiEnvironment,
    save: SavePreferences,
    started: Instant,
    /// Dropped with the root; the load finishes or is cancelled with it.
    _load: Option<Task<()>>,
}

impl LabRoot {
    fn new(
        snapshot_dir: PathBuf,
        environment: UiEnvironment,
        save: SavePreferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let verified = verify_bundled_faces(window, cx);
        let mut root = Self {
            stage: Stage::Loading,
            focus: cx.focus_handle(),
            environment,
            save,
            started: Instant::now(),
            _load: None,
        };
        match verified {
            Err(error) => {
                root.stage = Stage::Failed(
                    format!(
                        "The bundled fonts did not verify, so the Presenter will not open: {error}"
                    )
                    .into(),
                );
            }
            Ok(_) => {
                root._load = Some(cx.spawn_in(window, async move |this, cx| {
                    let loaded = cx
                        .background_executor()
                        .spawn(async move { read_snapshot(&DirectorySource::new(snapshot_dir)) })
                        .await;
                    this.update_in(cx, |root, window, cx| root.loaded(loaded, window, cx))
                        .ok();
                }));
            }
        }
        root
    }

    fn loaded(
        &mut self,
        loaded: Result<LoadedSnapshot, SnapshotLoadError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match loaded {
            Err(error) => {
                self.stage = Stage::Failed(format!("The snapshot did not load: {error}").into());
            }
            Ok(snapshot) => {
                let started = self.started;
                let clock: Rc<dyn Fn() -> Millis> =
                    Rc::new(move || Millis(started.elapsed().as_millis() as u64));
                let environment = self.environment;
                let save = self.save.clone();
                let view = cx.new(|cx| {
                    let mut view =
                        PresenterView::new(Arc::new(snapshot), clock, environment, window, cx);
                    view.set_save_preferences(save);
                    view
                });
                self.stage = Stage::Ready(view);
            }
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn send(&mut self, command: Command, cx: &mut Context<Self>) {
        if let Stage::Ready(view) = &self.stage {
            view.update(cx, |view, cx| {
                // A refusal is already on the status line.
                view.command(command, cx).ok();
            });
        }
    }
}

impl Focusable for LabRoot {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for LabRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let message = |text: SharedString| {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(px(scale.apply(tokens::spacing::SECTION)))
                .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                .child(text)
                .into_any_element()
        };
        let body = match &self.stage {
            Stage::Loading => message("Loading…".into()),
            Stage::Failed(text) => message(text.clone()),
            Stage::Ready(view) => div().size_full().child(view.clone()).into_any_element(),
        };
        div()
            .id("live-control-lab")
            .key_context("LiveControlLab")
            .track_focus(&self.focus)
            .size_full()
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .font_family(tokens::typography::OPERATOR_FAMILY)
            .on_action(cx.listener(|this, _: &PreviousCue, _, cx| this.send(Command::Previous, cx)))
            .on_action(cx.listener(|this, _: &NextCue, _, cx| this.send(Command::Next, cx)))
            .on_action(cx.listener(|this, _: &TakeLive, _, cx| this.send(Command::Take, cx)))
            .on_action(
                cx.listener(|this, _: &ToggleFreeze, _, cx| this.send(Command::ToggleFreeze, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleBlackout, _, cx| {
                this.send(Command::ToggleBlackout, cx);
            }))
            .on_action(
                cx.listener(|this, _: &EnableOutput, _, cx| this.send(Command::EnableOutput, cx)),
            )
            .on_action(cx.listener(|this, _: &SimulateDisplayLost, _, cx| {
                this.send(Command::DisplayLost, cx);
            }))
            .on_action(cx.listener(|this, _: &SimulateDisplayReturned, _, cx| {
                this.send(Command::DisplayReturned { recognized: true }, cx);
            }))
            .child(body)
    }
}

/// The worker that writes saved preferences, and the sender that feeds it.
struct PreferenceWriter {
    sender: Rc<RefCell<Option<mpsc::Sender<String>>>>,
    thread: JoinHandle<()>,
}

fn start_preference_writer(file: PreferencesFile) -> PreferenceWriter {
    let (sender, receiver) = mpsc::channel::<String>();
    let thread = std::thread::spawn(move || {
        // Ends when every sender is dropped.
        for contents in receiver {
            if let Err(error) = file.write(&contents) {
                eprintln!("live-control-lab: preferences were not saved: {error}");
            }
        }
    });
    PreferenceWriter {
        sender: Rc::new(RefCell::new(Some(sender))),
        thread,
    }
}

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("live-control-lab: {message}");
            eprintln!(
                "usage: live-control-lab [--size 1366x768] [--scale 100|125|150|175|200] [snapshot-dir]"
            );
            std::process::exit(2);
        }
    };

    let preferences_file = PreferencesFile::new(lab_dir().join("preferences"));
    // Before the app loop: a few bytes, read once, restoring the Grid/List choice.
    let restored = preferences_file
        .read()
        .ok()
        .flatten()
        .and_then(|contents| UiPreferences::decode(&contents).ok())
        .unwrap_or_default();
    let preferences = UiPreferences {
        scale: args.scale,
        ..restored
    };
    let mut environment = UiEnvironment::new(preferences, ResolvedAppearance::Dark);
    environment.apply(EnvironmentChange::EnterPresenter);

    let writer = start_preference_writer(preferences_file);
    let sender = writer.sender.clone();
    let save: SavePreferences = {
        let sender = writer.sender.clone();
        Rc::new(move |preferences: UiPreferences| {
            if let Some(sender) = sender.borrow().as_ref() {
                sender.send(preferences.encode()).ok();
            }
        })
    };

    application().run(move |cx: &mut App| {
        if let Err(error) = ui::init(cx) {
            eprintln!("live-control-lab: the UI could not start: {error}");
            cx.quit();
            return;
        }
        cx.bind_keys([
            KeyBinding::new("left", PreviousCue, None),
            KeyBinding::new("right", NextCue, None),
            KeyBinding::new("space", TakeLive, None),
            KeyBinding::new("f", ToggleFreeze, None),
            KeyBinding::new("b", ToggleBlackout, None),
            KeyBinding::new("e", EnableOutput, None),
            KeyBinding::new("d", SimulateDisplayLost, None),
            KeyBinding::new("r", SimulateDisplayReturned, None),
        ]);
        let bounds = Bounds::centered(None, size(px(args.width), px(args.height)), cx);
        let snapshot = args.snapshot.clone();
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(
                        format!("Ordo live-control lab (development only) · {KEY_HELP}").into(),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let root = LabRoot::new(snapshot, environment, save, window, cx);
                    window.focus(&root.focus_handle(cx), cx);
                    root
                })
            },
        );
        if let Err(error) = opened {
            eprintln!("live-control-lab: cannot open the window: {error:?}");
            cx.quit();
            return;
        }
        // Closing the window ends the lab: drop the preference sender first
        // so the writer drains, then quit.
        cx.on_window_closed(move |cx, _| {
            sender.borrow_mut().take();
            cx.quit();
        })
        .detach();
        cx.activate(true);
    });

    // Every sender is gone once the window closed; wait for the last save.
    writer.sender.borrow_mut().take();
    if writer.thread.join().is_err() {
        eprintln!("live-control-lab: the preference writer panicked");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn defaults_are_the_compact_reference_size_on_the_normal_mass() {
        let args = parse(&[]).unwrap();
        assert_eq!((args.width, args.height), (1366.0, 768.0));
        assert_eq!(args.scale, InterfaceScale::Percent100);
        assert!(
            args.snapshot
                .ends_with("tests/fixtures/presentations/normal-mass")
        );
    }

    #[test]
    fn size_scale_and_directory_are_read() {
        let args = parse(&["--size", "1920x1080", "--scale", "200", "some/dir"]).unwrap();
        assert_eq!((args.width, args.height), (1920.0, 1080.0));
        assert_eq!(args.scale, InterfaceScale::Percent200);
        assert_eq!(args.snapshot, PathBuf::from("some/dir"));
    }

    #[test]
    fn bad_arguments_are_refused_with_a_reason() {
        for bad in [
            &["--size"][..],
            &["--size", "wide"],
            &["--size", "10x10"],
            &["--scale", "110"],
            &["--frobnicate"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
