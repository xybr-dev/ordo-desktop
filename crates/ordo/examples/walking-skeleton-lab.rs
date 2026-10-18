//! Development-only walking-skeleton lab (Milestone 0 checks by hand).
//!
//! `cargo run -p ordo --example walking-skeleton-lab` composes the existing
//! Module Interfaces into one program an operator can drive on real hardware:
//! the real native runtime (operator shell, workers, congregation output,
//! supervision), a small Mass-shaped deck of the canonical rendering fixtures,
//! the pure Presenter Engine, and the Storage Module on a fresh temporary
//! database. It is a lab, not Presenter Mode: Presenter Mode, its Cue
//! timeline, and its output controls belong to Phase 1, and this program adds
//! no product behaviour to any crate.
//!
//! One lab window shows the title, Preview, Program (what the congregation
//! output is showing), the Cue position, a status line, and the key help.
//! Every Cue is composed on the runtime's canonical worker and rasterized on a
//! background thread at startup, so nothing renders on the GPUI thread. The
//! fixtures are embedded, so a packaged binary needs no source tree.
//!
//! Keys, with the lab window focused: `Right` / `Left` move Preview by one
//! Cue; `Up` / `Down` jump to the first Cue of the previous / next item;
//! `Space` takes Preview live; `F` toggles Freeze; `B` toggles Blackout;
//! `F6` assigns the only other display and enables output (black until
//! `Space`); `F8` re-reads the display topology; `F9` disables output; `N`
//! writes a note through the Storage Module while navigation continues.
//!
//! Closing the lab window closes the operator window too, which runs the
//! runtime's existing supervision and quits the whole app. Closing the
//! operator window does the same directly. Installation preferences and the
//! database live in one temporary directory that is removed on exit, so the
//! lab never touches real Parish data.

use std::{path::PathBuf, time::Duration};

use gpui::{
    App, AppContext as _, Bounds, Context, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyBinding, ParentElement as _, Render, StatefulInteractiveElement as _, Styled,
    Window, WindowBounds, WindowOptions, actions, div, px, rgb, size,
};
use gpui_platform::application;
use ordo::{
    canonical::CanonicalCommand, canonical_views, open_native_runtime_at,
    output::CongregationOutput,
};
use presentation::{
    OutputStatus, ProgramFrame,
    presenter::{PresenterCommand, PresenterEngine, PresenterState},
};
use rendering::{ForegroundImage, TargetDensity, TemplateContent, TemplateKind};
use storage::{Storage, StorageConfig};
use ui::{
    canonical_frame::FinishedFrame,
    environment::{ResolvedAppearance, UiEnvironment, UiPreferences},
    tokens,
};

actions!(
    walking_skeleton_lab,
    [
        NextCue,
        PreviousCue,
        PreviousItem,
        NextItem,
        TakeLive,
        ToggleFreeze,
        ToggleBlackout,
        EnableOutput,
        RefreshDisplays,
        DisableOutput,
        WriteNote,
    ]
);

const KEY_HELP: &str = "Right/Left Cue · Up/Down item · Space take live · F freeze · B blackout · F6 enable output · F8 refresh displays · F9 disable output · N storage write";

/// The Mass-shaped deck: each item and the rendering fixtures (by file stem
/// under `tests/fixtures/rendering/`) that become its Cues.
const DECK: &[(&str, &[&str])] = &[
    (
        "Entrance Song",
        &[
            "song-identification",
            "lyrics-english-two-line",
            "lyrics-tagalog-three-line",
        ],
    ),
    ("Penitential Act", &["response-cebuano"]),
    (
        "First Reading",
        &["reading-identification", "reading-passage-english"],
    ),
    ("Blackout", &["black"]),
];

/// Embedded fixture text, so the packaged binary never reads the source tree.
fn fixture_source(name: &str) -> &'static str {
    match name {
        "song-identification" => {
            include_str!("../../../tests/fixtures/rendering/song-identification.txt")
        }
        "lyrics-english-two-line" => {
            include_str!("../../../tests/fixtures/rendering/lyrics-english-two-line.txt")
        }
        "lyrics-tagalog-three-line" => {
            include_str!("../../../tests/fixtures/rendering/lyrics-tagalog-three-line.txt")
        }
        "response-cebuano" => {
            include_str!("../../../tests/fixtures/rendering/response-cebuano.txt")
        }
        "reading-identification" => {
            include_str!("../../../tests/fixtures/rendering/reading-identification.txt")
        }
        "reading-passage-english" => {
            include_str!("../../../tests/fixtures/rendering/reading-passage-english.txt")
        }
        "black" => include_str!("../../../tests/fixtures/rendering/black.txt"),
        other => panic!("fixture {other} is not embedded"),
    }
}

// Copied from `canonical-slide-lab.rs` (`parse_kind`, `parse_fixture`), which
// mirrors `rendering`'s golden harness; no public parser exists to reuse.
fn parse_kind(name: &str) -> TemplateKind {
    match name {
        "Lyrics" => TemplateKind::Lyrics,
        "Response" => TemplateKind::Response,
        "ReadingIdentification" => TemplateKind::ReadingIdentification,
        "ReadingPassage" => TemplateKind::ReadingPassage,
        "Psalm" => TemplateKind::Psalm,
        "SongIdentification" => TemplateKind::SongIdentification,
        "CelebrationTitle" => TemplateKind::CelebrationTitle,
        "GeneralText" => TemplateKind::GeneralText,
        "GeneralImage" => TemplateKind::GeneralImage,
        "Black" => TemplateKind::Black,
        _ => panic!("unknown template kind: {name}"),
    }
}

fn parse_fixture(name: &str, raw: &str) -> TemplateContent {
    let mut lines = raw.lines();
    let mut next = |what: &str| -> &str {
        lines
            .next()
            .unwrap_or_else(|| panic!("fixture {name} ends early: missing {what}"))
    };
    assert_eq!(
        next("magic"),
        "ordo-fixture/1",
        "fixture {name} has a bad magic"
    );
    let kind = parse_kind(
        next("kind")
            .strip_prefix("kind:")
            .unwrap_or_else(|| panic!("fixture {name} has a bad kind")),
    );
    let header_line = next("header");
    let header = if header_line == "header-absent" {
        None
    } else {
        Some(
            header_line
                .strip_prefix("header:")
                .unwrap_or_else(|| panic!("fixture {name} has a bad header"))
                .to_owned(),
        )
    };
    let body_count: usize = next("body count")
        .strip_prefix("body-lines:")
        .unwrap_or_else(|| panic!("fixture {name} has a bad body count"))
        .parse()
        .unwrap_or_else(|_| panic!("fixture {name} has a bad body count"));
    let mut body = Vec::with_capacity(body_count);
    for _ in 0..body_count {
        body.push(next("body line").to_owned());
    }
    let footer_line = next("footer");
    let footer = if footer_line == "footer-absent" {
        None
    } else {
        let footer_count: usize = footer_line
            .strip_prefix("footer-lines:")
            .unwrap_or_else(|| panic!("fixture {name} has a bad footer"))
            .parse()
            .unwrap_or_else(|_| panic!("fixture {name} has a bad footer"));
        let mut footer = Vec::with_capacity(footer_count);
        for _ in 0..footer_count {
            footer.push(next("footer line").to_owned());
        }
        Some(footer.join("\n"))
    };
    let image_line = next("image");
    let image = if image_line == "image-absent" {
        None
    } else {
        let id = image_line
            .strip_prefix("image:")
            .unwrap_or_else(|| panic!("fixture {name} has a bad image"));
        assert!(!id.is_empty(), "fixture {name} has an empty image id");
        let size = next("image size")
            .strip_prefix("image-size:")
            .unwrap_or_else(|| panic!("fixture {name} has a bad image size"))
            .to_owned();
        let (width, height) = size
            .split_once('x')
            .unwrap_or_else(|| panic!("fixture {name} has a bad image size"));
        Some(ForegroundImage {
            id: id.to_owned(),
            natural_width: width
                .parse()
                .unwrap_or_else(|_| panic!("fixture {name} has a bad image size")),
            natural_height: height
                .parse()
                .unwrap_or_else(|_| panic!("fixture {name} has a bad image size")),
        })
    };
    assert!(
        lines.next().is_none(),
        "fixture {name} has trailing content"
    );
    TemplateContent {
        kind,
        header,
        body: body.join("\n"),
        footer,
        image,
    }
}

// ---------------------------------------------------------------------------
// Pure logic: deck shape, item navigation, and Program selection.
// ---------------------------------------------------------------------------

/// Cue count of each item, in deck order.
fn item_cue_counts() -> Vec<usize> {
    DECK.iter().map(|(_, cues)| cues.len()).collect()
}

/// Which item holds Cue `cue`, as `(item index, offset within the item)`.
/// A Cue past the end maps to the last Cue of the last item; an empty deck
/// maps to `(0, 0)`.
fn locate(counts: &[usize], cue: usize) -> (usize, usize) {
    let mut start = 0;
    for (item, &count) in counts.iter().enumerate() {
        if cue < start + count {
            return (item, cue - start);
        }
        start += count;
    }
    match counts.len().checked_sub(1) {
        Some(last) => (last, counts[last].saturating_sub(1)),
        None => (0, 0),
    }
}

/// The index of the first Cue of `item`, clamped to the last item.
fn item_start(counts: &[usize], item: usize) -> usize {
    let item = item.min(counts.len().saturating_sub(1));
    counts.iter().take(item).sum()
}

/// `GoTo` target of the `Up` key: the first Cue of the previous item, or of
/// the first item when Preview is already inside it.
fn up_target(counts: &[usize], preview: usize) -> usize {
    let (item, _) = locate(counts, preview);
    item_start(counts, item.saturating_sub(1))
}

/// `GoTo` target of the `Down` key: the first Cue of the next item, or of the
/// last item when Preview is already inside it.
fn down_target(counts: &[usize], preview: usize) -> usize {
    let (item, _) = locate(counts, preview);
    item_start(counts, item + 1)
}

/// What the congregation output should be showing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProgramTarget {
    Black,
    Cue(usize),
}

/// The visible Program content for `state`: black when blacked out or when
/// nothing has been taken.
fn visible_target(state: &PresenterState) -> ProgramTarget {
    match state.program_visible() {
        Some(cue) => ProgramTarget::Cue(cue),
        None => ProgramTarget::Black,
    }
}

/// Whether `command` must send a frame to the output. Program's visible
/// content changing sends; so does every explicit `TakeLive`, because the
/// output may have been black or rejected while the engine state did not
/// change (the restore path after enabling or after a display loss).
fn program_update(
    before: &PresenterState,
    after: &PresenterState,
    command: PresenterCommand,
) -> Option<ProgramTarget> {
    let changed = visible_target(before) != visible_target(after);
    (changed || command == PresenterCommand::TakeLive).then(|| visible_target(after))
}

/// "item / cue" text for a Preview index, one-based.
fn position_label(counts: &[usize], preview: usize) -> String {
    let (item, offset) = locate(counts, preview);
    let (item_total, cue_total) = (counts.len(), counts.get(item).copied().unwrap_or(0));
    let name = DECK.get(item).map_or("(empty)", |(name, _)| name);
    format!(
        "Item {}/{item_total} {name} · Cue {}/{cue_total}",
        item + 1,
        offset + 1
    )
}

// ---------------------------------------------------------------------------
// Composition: worker compose, then rasterize and finish off the GPUI thread.
// ---------------------------------------------------------------------------

/// One Cue's finished pixels: the operator viewport frame and the Program
/// frame the congregation output takes.
struct CueFrames {
    finished: FinishedFrame,
    program: ProgramFrame,
}

type CueResult = (usize, Result<CueFrames, String>);

/// Compose every Cue in order on the runtime's canonical worker, then
/// rasterize and finish on this thread. The GPUI thread only receives results.
fn compose_cues(
    contents: Vec<TemplateContent>,
    canonical: std::sync::mpsc::Sender<CanonicalCommand>,
    environment: UiEnvironment,
    window_scale: f32,
    results: async_channel::Sender<CueResult>,
) {
    let spawned = std::thread::Builder::new()
        .name("ordo-skeleton-raster".to_owned())
        .spawn(move || {
            for (index, content) in contents.into_iter().enumerate() {
                let outcome = compose_one(&canonical, environment, window_scale, content);
                if results.send_blocking((index, outcome)).is_err() {
                    break;
                }
            }
        });
    if let Err(error) = spawned {
        eprintln!("cannot start the raster thread: {error}");
    }
}

fn compose_one(
    canonical: &std::sync::mpsc::Sender<CanonicalCommand>,
    environment: UiEnvironment,
    window_scale: f32,
    content: TemplateContent,
) -> Result<CueFrames, String> {
    let (reply, answer) = std::sync::mpsc::channel();
    canonical
        .send(CanonicalCommand::Compose {
            environment,
            window_scale,
            content,
            reply,
        })
        .map_err(|_| "the canonical worker stopped".to_owned())?;
    let (frame, _diagnostics) = answer
        .recv()
        .map_err(|_| "the canonical worker stopped".to_owned())?
        .map_err(|error| format!("composition failed: {error}"))?;
    // The deck holds no image fixtures, so no foreground bytes are needed.
    let raster = rendering::rasterize(
        &frame,
        TargetDensity::SCALE_100,
        &std::collections::HashMap::new(),
    )
    .map_err(|error| format!("raster failed: {error}"))?;
    let finished = canonical_views::finish_frame(&raster)
        .ok_or_else(|| "the raster could not be finished".to_owned())?;
    let program = ProgramFrame::from_raster(0, raster)
        .map_err(|error| format!("program frame failed: {error:?}"))?;
    Ok(CueFrames { finished, program })
}

// ---------------------------------------------------------------------------
// The lab window.
// ---------------------------------------------------------------------------

struct SkeletonLab {
    counts: Vec<usize>,
    engine: PresenterEngine,
    cues: Vec<Option<CueFrames>>,
    composed: usize,
    output: gpui::Entity<CongregationOutput>,
    /// What the output last accepted. Reset to black when output is enabled
    /// or disabled; reads as black whenever output is not connected.
    on_air: ProgramTarget,
    storage: Option<Storage>,
    storage_state: String,
    parish_named: bool,
    writes: u32,
    notes: Option<u64>,
    message: String,
    environment: UiEnvironment,
    focus_handle: FocusHandle,
}

impl SkeletonLab {
    fn new(
        output: gpui::Entity<CongregationOutput>,
        results: async_channel::Receiver<CueResult>,
        db_path: PathBuf,
        cue_total: usize,
        cx: &mut Context<Self>,
    ) -> Self {
        let storage = Storage::spawn(StorageConfig::new(db_path));
        let ready = storage.ready();
        cx.spawn(async move |this, cx| {
            let result = ready.await;
            let _ = this.update(cx, |this, cx| {
                this.storage_state = match result {
                    Ok(status) => format!("ready (schema v{})", status.version),
                    Err(error) => format!("open refused: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
        cx.spawn(async move |this, cx| {
            while let Ok((index, outcome)) = results.recv().await {
                let alive = this.update(cx, |this, cx| {
                    match outcome {
                        Ok(frames) => {
                            this.cues[index] = Some(frames);
                            this.composed += 1;
                        }
                        Err(error) => this.message = format!("Cue {} failed: {error}", index + 1),
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.observe(&output, |_, _, cx| cx.notify()).detach();
        let counts = item_cue_counts();
        Self {
            engine: PresenterEngine::new(counts.iter().sum()),
            counts,
            cues: (0..cue_total).map(|_| None).collect(),
            composed: 0,
            output,
            on_air: ProgramTarget::Black,
            storage: Some(storage),
            storage_state: "opening".to_owned(),
            parish_named: false,
            writes: 0,
            notes: None,
            message: "Press F6 to enable output, then Space to take a Cue live.".to_owned(),
            environment: UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light),
            focus_handle: cx.focus_handle(),
        }
    }

    fn command(&mut self, command: PresenterCommand, cx: &mut Context<Self>) {
        let before = *self.engine.state();
        let after = *self.engine.apply(command);
        if let Some(target) = program_update(&before, &after, command) {
            self.send_program(target, cx);
        }
        cx.notify();
    }

    fn send_program(&mut self, target: ProgramTarget, cx: &mut Context<Self>) {
        let frame = match target {
            ProgramTarget::Black => ProgramFrame::black(0),
            ProgramTarget::Cue(cue) => match self.cues.get(cue).and_then(Option::as_ref) {
                Some(frames) => frames.program.clone(),
                None => {
                    self.message =
                        format!("Cue {} is not composed yet; Program unchanged.", cue + 1);
                    return;
                }
            },
        };
        let outcome = self
            .output
            .update(cx, |output, cx| output.take_live(frame, cx));
        match outcome {
            Ok(()) => {
                self.on_air = target;
                self.message = match target {
                    ProgramTarget::Black => "Program: black.".to_owned(),
                    ProgramTarget::Cue(cue) => format!("Program: Cue {}.", cue + 1),
                };
            }
            Err(error) => self.message = format!("Program change rejected: {error:?}"),
        }
    }

    fn go_to(&mut self, target: usize, cx: &mut Context<Self>) {
        self.command(PresenterCommand::GoTo(target), cx);
    }

    fn enable_output(&mut self, cx: &mut Context<Self>) {
        let (notes, enabled) = self.output.update(cx, |output, cx| {
            output.refresh_displays(cx);
            let mut notes = Vec::new();
            match output.display_control().suggestion() {
                Some(candidate) => {
                    if let Err(error) = output.assign(candidate, cx) {
                        notes.push(format!("assign failed: {error:?}"));
                    }
                }
                None => notes.push("no single non-operator display to assign".to_owned()),
            }
            let enabled = match output.enable_output(cx) {
                Ok(_) => true,
                Err(error) => {
                    notes.push(format!("enable failed: {error:?}"));
                    false
                }
            };
            (notes, enabled)
        });
        if enabled {
            // Enabling always starts the congregation on black; content only
            // appears after an explicit Space.
            self.on_air = ProgramTarget::Black;
            self.message = "Output enabled, Program black. Press Space to take live.".to_owned();
        } else {
            self.message = notes.join("; ");
        }
        cx.notify();
    }

    fn refresh_displays(&mut self, cx: &mut Context<Self>) {
        let events = self
            .output
            .update(cx, |output, cx| output.refresh_displays(cx));
        self.message = format!("Displays re-read: {events:?}");
        cx.notify();
    }

    fn disable_output(&mut self, cx: &mut Context<Self>) {
        self.output
            .update(cx, |output, cx| output.disable_output(cx));
        self.on_air = ProgramTarget::Black;
        self.message = "Output disabled.".to_owned();
        cx.notify();
    }

    /// One Storage write (naming the parish first when needed, because notes
    /// reference the parish profile), awaited off the GPUI thread.
    fn write_note(&mut self, cx: &mut Context<Self>) {
        let Some(storage) = self.storage.as_ref() else {
            return;
        };
        self.writes += 1;
        let name = (!self.parish_named).then(|| storage.set_parish_name("Walking Skeleton".into()));
        let note = storage.add_parish_note(format!("lab note {}", self.writes));
        let count = storage.parish_note_count();
        cx.spawn(async move |this, cx| {
            let named = match name {
                Some(pending) => Some(pending.await),
                None => None,
            };
            let noted = note.await;
            let counted = count.await;
            let _ = this.update(cx, |this, cx| {
                if matches!(named, Some(Ok(()))) {
                    this.parish_named = true;
                }
                match (named, noted) {
                    (Some(Err(error)), _) | (_, Err(error)) => {
                        this.storage_state = format!("write failed: {error}");
                    }
                    (_, Ok(_)) => this.storage_state = "write ok".to_owned(),
                }
                if let Ok(count) = counted {
                    this.notes = Some(count);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn status_text(&self, cx: &App) -> (String, String) {
        let output = self.output.read(cx);
        let control = output.display_control();
        let state = self.engine.state();
        let flag = |on: bool| if on { "ON" } else { "off" };
        let notes = self
            .notes
            .map_or_else(|| "?".to_owned(), |count| count.to_string());
        (
            format!(
                "Output {:?} · {:?} · assigned {:?} · Freeze {} · Blackout {}",
                control.output(),
                output.congregation().output_status(),
                control.assignment(),
                flag(state.frozen()),
                flag(state.blackout()),
            ),
            format!(
                "Storage {} · writes {} · notes {} · {}",
                self.storage_state, self.writes, notes, self.message
            ),
        )
    }

    fn program_view(&self, cx: &App) -> gpui::AnyElement {
        let connected =
            self.output.read(cx).congregation().output_status() == OutputStatus::Connected;
        let shown = if connected {
            self.on_air
        } else {
            ProgramTarget::Black
        };
        let frames = match shown {
            ProgramTarget::Cue(cue) => self.cues.get(cue).and_then(Option::as_ref),
            ProgramTarget::Black => None,
        };
        match frames {
            Some(frames) => canonical_views::program_frame(&frames.finished, self.environment),
            None => {
                let width = self.environment.preferences().scale.apply(720.0);
                div()
                    .w(px(width))
                    .h(px(width * 9.0 / 16.0))
                    .bg(rgb(0x000000))
                    .into_any_element()
            }
        }
    }
}

impl Focusable for SkeletonLab {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SkeletonLab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let state = *self.engine.state();
        let (output_line, storage_line) = self.status_text(cx);
        let preview = match self.cues.get(state.preview()).and_then(Option::as_ref) {
            Some(frames) => canonical_views::preview_frame(&frames.finished, environment),
            None => div()
                .w(px(scale.apply(720.0)))
                .h(px(scale.apply(720.0) * 9.0 / 16.0))
                .bg(rgb(palette.surface))
                .child("composing…")
                .into_any_element(),
        };
        let progress = format!(
            "Composed {}/{} Cues · {}",
            self.composed,
            self.cues.len(),
            position_label(&self.counts, state.preview())
        );
        let counts = self.counts.clone();

        div()
            .id("walking-skeleton-lab")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .text_size(px(scale.apply(tokens::typography::BODY.size)))
            .on_action(cx.listener(|this, _: &NextCue, _, cx| {
                this.command(PresenterCommand::Next, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviousCue, _, cx| {
                this.command(PresenterCommand::Previous, cx);
            }))
            .on_action(cx.listener(move |this, _: &PreviousItem, _, cx| {
                let target = up_target(&counts, this.engine.state().preview());
                this.go_to(target, cx);
            }))
            .on_action(cx.listener(|this, _: &NextItem, _, cx| {
                let target = down_target(&this.counts, this.engine.state().preview());
                this.go_to(target, cx);
            }))
            .on_action(cx.listener(|this, _: &TakeLive, _, cx| {
                this.command(PresenterCommand::TakeLive, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleFreeze, _, cx| {
                this.command(PresenterCommand::ToggleFreeze, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleBlackout, _, cx| {
                this.command(PresenterCommand::ToggleBlackout, cx);
            }))
            .on_action(cx.listener(|this, _: &EnableOutput, _, cx| this.enable_output(cx)))
            .on_action(cx.listener(|this, _: &RefreshDisplays, _, cx| this.refresh_displays(cx)))
            .on_action(cx.listener(|this, _: &DisableOutput, _, cx| this.disable_output(cx)))
            .on_action(cx.listener(|this, _: &WriteNote, _, cx| this.write_note(cx)))
            .child(
                div()
                    .flex_none()
                    .p(px(scale.apply(tokens::spacing::GROUP)))
                    .border_b(px(tokens::border::SEPARATOR))
                    .border_color(rgb(palette.border))
                    .child(
                        div()
                            .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                            .child(
                                "Ordo walking-skeleton lab (development only, not Presenter Mode)",
                            ),
                    )
                    .child(div().child(progress))
                    .child(div().child(output_line))
                    .child(div().child(storage_line))
                    .child(div().text_color(rgb(palette.text_muted)).child(KEY_HELP)),
            )
            .child(
                div()
                    .id("walking-skeleton-stage")
                    .flex_1()
                    .overflow_scroll()
                    .p(px(scale.apply(tokens::spacing::GROUP)))
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(scale.apply(tokens::spacing::SECTION)))
                    .child(
                        div()
                            .flex_none()
                            .child(div().child("Preview"))
                            .child(preview),
                    )
                    .child(
                        div()
                            .flex_none()
                            .child(div().child("Program (on the congregation display)"))
                            .child(self.program_view(cx)),
                    ),
            )
    }
}

/// The lab's one temporary directory (preferences file and database).
fn lab_dir() -> PathBuf {
    std::env::temp_dir().join(format!("ordo-walking-skeleton-lab-{}", std::process::id()))
}

/// Remove the temporary directory, retrying briefly because the storage
/// worker may still be closing its database as the process winds down.
fn remove_lab_dir(dir: &std::path::Path) {
    for _ in 0..40 {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    eprintln!("could not remove {}", dir.display());
}

fn main() {
    let dir = lab_dir();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {error}", dir.display());
        std::process::exit(1);
    }
    let preferences = dir.join("preferences");
    let database = dir.join("parish.sqlite3");
    let contents: Vec<TemplateContent> = DECK
        .iter()
        .flat_map(|(_, cues)| cues.iter())
        .map(|name| parse_fixture(name, fixture_source(name)))
        .collect();

    application().run(move |cx: &mut App| {
        let runtime =
            open_native_runtime_at(cx, &preferences).expect("the native runtime should open");
        let output = runtime.output();
        let operator = runtime.operator();
        cx.bind_keys([
            KeyBinding::new("right", NextCue, None),
            KeyBinding::new("left", PreviousCue, None),
            KeyBinding::new("up", PreviousItem, None),
            KeyBinding::new("down", NextItem, None),
            KeyBinding::new("space", TakeLive, None),
            KeyBinding::new("f", ToggleFreeze, None),
            KeyBinding::new("b", ToggleBlackout, None),
            KeyBinding::new("f6", EnableOutput, None),
            KeyBinding::new("f8", RefreshDisplays, None),
            KeyBinding::new("f9", DisableOutput, None),
            KeyBinding::new("n", WriteNote, None),
        ]);

        let (results_tx, results_rx) = async_channel::unbounded();
        let cue_total = contents.len();
        let database = database.clone();
        let bounds = Bounds::centered(None, size(px(1560.0), px(900.0)), cx);
        let lab = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let lab = SkeletonLab::new(output, results_rx, database, cue_total, cx);
                    window.focus(&lab.focus_handle(cx), cx);
                    lab
                })
            },
        );
        let lab = match lab {
            Ok(lab) => lab,
            Err(error) => {
                eprintln!("cannot open the lab window: {error:?}");
                cx.quit();
                return;
            }
        };
        let window_scale = lab
            .update(cx, |_, window, _| window.scale_factor())
            .unwrap_or(1.0);
        // Composition runs on the runtime's canonical worker; rasterizing
        // and finishing run on a background thread, never the GPUI thread.
        compose_cues(
            contents.clone(),
            runtime.canonical_commands(),
            UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light),
            window_scale,
            results_tx,
        );

        // Closing the lab window closes the operator window, which runs the
        // runtime's supervision (output shutdown, worker shutdown, quit).
        let lab_id = lab.window_id();
        cx.on_window_closed(move |cx, closed| {
            if closed == lab_id {
                cx.defer(move |cx| {
                    operator
                        .update(cx, |_, window, _| window.remove_window())
                        .ok();
                });
            }
        })
        .detach();
        cx.activate(true);
    });
    remove_lab_dir(&dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_after(cues: usize, commands: &[PresenterCommand]) -> (PresenterState, PresenterState) {
        let mut engine = PresenterEngine::new(cues);
        let mut before = *engine.state();
        let mut after = before;
        for &command in commands {
            before = *engine.state();
            after = *engine.apply(command);
        }
        (before, after)
    }

    #[test]
    fn every_deck_fixture_parses_and_the_deck_has_the_planned_shape() {
        let counts = item_cue_counts();
        assert_eq!(counts, vec![3, 1, 2, 1]);
        for (_, cues) in DECK {
            for name in *cues {
                let content = parse_fixture(name, fixture_source(name));
                assert!(content.image.is_none(), "{name} would need asset bytes");
            }
        }
    }

    #[test]
    fn locate_maps_cues_to_items_and_clamps_past_the_end() {
        let counts = [3, 1, 2, 1];
        assert_eq!(locate(&counts, 0), (0, 0));
        assert_eq!(locate(&counts, 2), (0, 2));
        assert_eq!(locate(&counts, 3), (1, 0));
        assert_eq!(locate(&counts, 4), (2, 0));
        assert_eq!(locate(&counts, 6), (3, 0));
        assert_eq!(locate(&counts, 99), (3, 0));
        assert_eq!(locate(&[], 5), (0, 0));
    }

    #[test]
    fn up_and_down_go_to_the_first_cue_of_the_neighbouring_item() {
        let counts = [3, 1, 2, 1];
        // Down from inside the first item reaches the next item's first Cue.
        assert_eq!(down_target(&counts, 0), 3);
        assert_eq!(down_target(&counts, 2), 3);
        assert_eq!(down_target(&counts, 3), 4);
        assert_eq!(down_target(&counts, 5), 6);
        // Down on the last item stays on that item's first Cue.
        assert_eq!(down_target(&counts, 6), 6);
        // Up from a later item reaches the previous item's first Cue.
        assert_eq!(up_target(&counts, 6), 4);
        assert_eq!(up_target(&counts, 5), 3);
        assert_eq!(up_target(&counts, 4), 3);
        assert_eq!(up_target(&counts, 3), 0);
        // Up on the first item stays on its first Cue, from any Cue in it.
        assert_eq!(up_target(&counts, 2), 0);
        assert_eq!(up_target(&counts, 0), 0);
        assert_eq!(item_start(&counts, 99), 6);
    }

    #[test]
    fn navigation_alone_never_updates_program() {
        let (before, after) = state_after(7, &[PresenterCommand::Next]);
        assert_eq!(
            program_update(&before, &after, PresenterCommand::Next),
            None
        );
    }

    #[test]
    fn take_live_selects_the_previewed_cue_even_when_state_repeats() {
        let (before, after) = state_after(7, &[PresenterCommand::Next, PresenterCommand::TakeLive]);
        assert_eq!(
            program_update(&before, &after, PresenterCommand::TakeLive),
            Some(ProgramTarget::Cue(1))
        );
        // A second take of the same Cue changes no state but still sends,
        // which is the restore path after enabling output.
        let mut engine = PresenterEngine::new(7);
        engine.apply(PresenterCommand::TakeLive);
        let before = *engine.state();
        let after = *engine.apply(PresenterCommand::TakeLive);
        assert_eq!(before, after);
        assert_eq!(
            program_update(&before, &after, PresenterCommand::TakeLive),
            Some(ProgramTarget::Cue(0))
        );
    }

    #[test]
    fn blackout_sends_black_and_release_restores_the_cue() {
        let mut engine = PresenterEngine::new(7);
        engine.apply(PresenterCommand::TakeLive);
        let before = *engine.state();
        let after = *engine.apply(PresenterCommand::ToggleBlackout);
        assert_eq!(
            program_update(&before, &after, PresenterCommand::ToggleBlackout),
            Some(ProgramTarget::Black)
        );
        let before = after;
        let after = *engine.apply(PresenterCommand::ToggleBlackout);
        assert_eq!(
            program_update(&before, &after, PresenterCommand::ToggleBlackout),
            Some(ProgramTarget::Cue(0))
        );
    }

    #[test]
    fn freeze_and_blackout_before_any_take_leave_the_output_alone() {
        let mut engine = PresenterEngine::new(7);
        let before = *engine.state();
        let after = *engine.apply(PresenterCommand::ToggleFreeze);
        assert_eq!(
            program_update(&before, &after, PresenterCommand::ToggleFreeze),
            None
        );
        let before = after;
        let after = *engine.apply(PresenterCommand::ToggleBlackout);
        assert_eq!(
            program_update(&before, &after, PresenterCommand::ToggleBlackout),
            None
        );
    }

    #[test]
    fn position_label_is_one_based_item_and_cue() {
        let counts = item_cue_counts();
        assert_eq!(
            position_label(&counts, 1),
            "Item 1/4 Entrance Song · Cue 2/3"
        );
        assert_eq!(position_label(&counts, 6), "Item 4/4 Blackout · Cue 1/1");
    }
}
