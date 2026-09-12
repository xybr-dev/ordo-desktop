//! Development-only canonical slide lab (T0.4 PR 9).
//!
//! `cargo run -p ordo --example canonical-slide-lab` composes the PR 6
//! fixtures through the real `rendering` path and shows them through the same
//! `ordo::canonical_views` viewports the timeline, Inspector, Preview, and
//! Program surfaces use. It is not a second renderer and not an application
//! workflow: composition and rasterization run on a background thread, so
//! rendering never blocks the GPUI thread, and the lab owns no shell,
//! navigation, or Preview-to-Program behavior. It lives here rather than in
//! `ui-lab`, whose `ui`-only dependency contract cannot reach `rendering`.
//!
//! Keys: `F2` cycles the fixture (every template, the label-off and black
//! variants, representative overflow warnings), `F3` the viewport preset,
//! `F4` the Interface scale, `F5` the target density. The renderer's
//! diagnostics ride beside the scaled frame.

use std::{collections::HashMap, path::PathBuf, sync::Arc, thread};

use async_channel::{Receiver, Sender};
use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, Focusable, IntoElement, KeyBinding,
    ParentElement, Render, RenderImage, Styled, Window, WindowBounds, WindowOptions, actions, div,
    prelude::*, px, rgb, size,
};
use gpui_platform::application;
use ordo::canonical_views;
use rendering::{Diagnostics, TargetDensity, TemplateContent, TemplateKind};
use ui::{
    canonical_frame::FinishedFrame,
    environment::{
        InterfaceScale, PreparationAppearance, ResolvedAppearance, UiEnvironment, UiPreferences,
    },
    tokens,
};

actions!(
    canonical_lab,
    [NextFixture, NextViewport, NextScale, NextDensity]
);

const SCALES: [InterfaceScale; 5] = [
    InterfaceScale::Percent100,
    InterfaceScale::Percent125,
    InterfaceScale::Percent150,
    InterfaceScale::Percent175,
    InterfaceScale::Percent200,
];

const DENSITIES: [u16; 5] = [100, 125, 150, 175, 200];

const VIEWPORTS: [(f32, f32, &str); 2] = [
    (1366.0, 768.0, "1366x768 Compact"),
    (1920.0, 1080.0, "1920x1080 Wide"),
];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/rendering")
}

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

/// The `ordo-fixture/1` line format `rendering`'s golden harness reads, parsed
/// the same way: raw header/body/footer lines in fixed order, `header-absent`
/// distinct from an empty `header:`, image identity plus natural size.
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
        Some(rendering::ForegroundImage {
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

fn load_fixtures() -> Vec<(String, TemplateContent)> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(fixtures_dir()).expect("fixtures dir must exist") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_some_and(|extension| extension == "txt") {
            names.push(path);
        }
    }
    names.sort();
    assert!(
        !names.is_empty(),
        "expected fixtures in tests/fixtures/rendering/"
    );
    names
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .expect("fixture needs a stem")
                .to_string_lossy()
                .into_owned();
            let raw = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("fixture {} must be readable: {error}", path.display())
            });
            (name.clone(), parse_fixture(&name, &raw))
        })
        .collect()
}

/// Stand-in foreground bytes for the GeneralImage fixtures, through the same
/// qualified `tiny-skia` edge the raster tests use. The lab reviews geometry
/// and composition here, not asset content.
fn solid_png(width: u32, height: u32) -> Vec<u8> {
    let mut pixmap =
        tiny_skia::Pixmap::new(width.max(1), height.max(1)).expect("lab pixmap must exist");
    pixmap.fill(tiny_skia::Color::from_rgba8(128, 128, 128, 255));
    pixmap.encode_png().expect("lab png must encode")
}

struct Request {
    sequence: u64,
    key: (usize, u16),
    content: TemplateContent,
    environment: UiEnvironment,
    window_scale: f32,
    reply: Sender<Rendered>,
}

#[derive(Clone)]
struct Rendered {
    sequence: u64,
    key: (usize, u16),
    frame: FinishedFrame,
    diagnostics: Diagnostics,
    // Gallery-manifest reads only; the live lab never displays these counts.
    #[allow(dead_code)]
    body_lines: usize,
    #[allow(dead_code)]
    footer_lines: usize,
}

/// The lab's raster stage: composition runs on the production canonical
/// worker behind `CanonicalCommand::Compose` — the same seam the packaged
/// runtime owns, carrying live operator state and the window scale — and
/// this thread rasterizes and finishes that payload, so rendering never
/// blocks the GPUI thread. Density reaches only the raster extent; the
/// logical composition is density-independent by construction.
fn spawn_worker(
    requests: Receiver<Request>,
    canonical: std::sync::mpsc::Sender<ordo::canonical::CanonicalCommand>,
) {
    thread::Builder::new()
        .name("ordo-lab-raster".to_owned())
        .spawn(move || {
            while let Ok(request) = requests.recv_blocking() {
                // Coalesce rapid selector changes: a dev-build render costs up
                // to seconds, so mashing keys would otherwise queue expensive
                // rasters faster than they drain and the display would look
                // frozen. Only the newest pending request renders; skipped
                // replies are dropped by the view's sequence guard.
                let mut request = request;
                while let Ok(newer) = requests.try_recv() {
                    request = newer;
                }
                let density =
                    TargetDensity::from_percent(request.key.1).unwrap_or(TargetDensity::SCALE_100);
                let (compose_tx, compose_rx) = std::sync::mpsc::channel();
                if canonical
                    .send(ordo::canonical::CanonicalCommand::Compose {
                        environment: request.environment,
                        window_scale: request.window_scale,
                        content: request.content,
                        reply: compose_tx,
                    })
                    .is_err()
                {
                    break;
                }
                let rendered = match compose_rx.recv() {
                    Ok(Ok((frame, diagnostics))) => {
                        let (body_lines, footer_lines) =
                            (frame.body_lines().len(), frame.footer_lines().len());
                        let mut images = HashMap::new();
                        if let Some(placed) = frame.image() {
                            images.insert(
                                placed.id.clone(),
                                solid_png(placed.bounds.width.max(1), placed.bounds.height.max(1)),
                            );
                        }
                        match rendering::rasterize(&frame, density, &images) {
                            Ok(raster) => {
                                canonical_views::finish_frame(&raster).map(|frame| Rendered {
                                    sequence: request.sequence,
                                    key: request.key,
                                    frame,
                                    diagnostics,
                                    body_lines,
                                    footer_lines,
                                })
                            }
                            Err(error) => {
                                eprintln!("lab raster failed: {error}");
                                None
                            }
                        }
                    }
                    Ok(Err(error)) => {
                        eprintln!("lab composition failed: {error}");
                        None
                    }
                    Err(_) => break,
                };
                if let Some(rendered) = rendered
                    && request.reply.send_blocking(rendered).is_err()
                {
                    break;
                }
            }
        })
        .expect("lab raster thread must start");
}

struct LabView {
    fixtures: Vec<(String, TemplateContent)>,
    fixture_idx: usize,
    viewport_idx: usize,
    scale_idx: usize,
    density_idx: usize,
    sequence: u64,
    rendered: Option<Rendered>,
    /// Finished pixels by (fixture, density): the key space is bounded by
    /// the fixture matrix times the five densities, so revisits never
    /// re-render and need no eviction. These are small PNG bytes; the
    /// expensive decoded copy plus GPU tile is bounded separately by
    /// `displayed`, which retires the outgoing slide.
    cache: HashMap<(usize, u16), Rendered>,
    displayed: Option<DisplayedImage>,
    /// The live window scale, re-read on every selector: it crosses the
    /// production boundary with the environment on each request, so the
    /// lab exercises the same input derivation as the packaged runtime
    /// (which spends neither value in composition).
    window_scale: f32,
    requests: Sender<Request>,
    focus_handle: FocusHandle,
}

/// The slide currently on screen with its decoded upload, mirroring Zed's
/// image viewer: retiring the outgoing pair (decoded copy plus atlas tile)
/// as soon as the incoming slide decodes bounds viewport memory to one
/// variant, instead of every variant ever displayed, which neither GPUI
/// cache evicts on its own.
struct DisplayedImage {
    source: Arc<gpui::Image>,
    raster: Arc<RenderImage>,
}

impl LabView {
    fn new(
        fixtures: Vec<(String, TemplateContent)>,
        requests: Sender<Request>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            fixtures,
            fixture_idx: 0,
            viewport_idx: 0,
            scale_idx: 0,
            density_idx: 0,
            sequence: 0,
            rendered: None,
            cache: HashMap::new(),
            displayed: None,
            window_scale: window.scale_factor(),
            requests,
            focus_handle: cx.focus_handle(),
        };
        view.request(cx);
        view
    }

    fn environment(&self) -> UiEnvironment {
        UiEnvironment::new(
            UiPreferences {
                appearance: PreparationAppearance::System,
                scale: SCALES[self.scale_idx],
            },
            ResolvedAppearance::Light,
        )
    }

    /// Show the current selection: a cached (fixture, density) pair displays
    /// synchronously, otherwise the worker composes it. Replies carry the
    /// request sequence, so a stale composition that finishes late cannot
    /// overwrite a newer one; the sequence always advances, even on a hit,
    /// so a late reply can never collide with a later request.
    fn request(&mut self, cx: &mut Context<Self>) {
        self.sequence += 1;
        let sequence = self.sequence;
        let key = (self.fixture_idx, DENSITIES[self.density_idx]);
        if let Some(rendered) = self.cache.get(&key) {
            self.rendered = Some(rendered.clone());
            cx.notify();
            return;
        }
        let content = self.fixtures[self.fixture_idx].1.clone();
        let environment = self.environment();
        let window_scale = self.window_scale;
        let (reply_tx, reply_rx) = async_channel::bounded(1);
        let requests = self.requests.clone();
        cx.spawn(async move |view, cx| {
            if requests
                .send(Request {
                    sequence,
                    key,
                    content,
                    environment,
                    window_scale,
                    reply: reply_tx,
                })
                .await
                .is_err()
            {
                return;
            }
            let Ok(rendered) = reply_rx.recv().await else {
                return;
            };
            view.update(cx, |view: &mut LabView, cx| {
                view.cache.insert(rendered.key, rendered.clone());
                if rendered.sequence == view.sequence {
                    view.rendered = Some(rendered);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Retire the outgoing slide's decoded copy and atlas tile once the
    /// incoming slide has decoded and can take the screen. Waiting for the
    /// decode keeps the previous slide up during the load gap instead of
    /// flashing black; swapping only on a new image id keeps chrome-only
    /// selectors free. This must go through `use_render_image` (like Zed's
    /// image viewer and the `img` elements below), not `get_render_image`:
    /// `get` creates the asset task without scheduling the on-next-frame
    /// notify, stealing `is_first` from the viewports' `use_asset` calls in
    /// the same frame, so a fresh decode would complete with no re-render
    /// scheduled and the viewport would stay black.
    fn retire_outgoing_upload(
        &mut self,
        current: Option<Arc<gpui::Image>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = current else { return };
        let already = self
            .displayed
            .as_ref()
            .is_some_and(|displayed| displayed.source.id() == current.id());
        if already {
            return;
        }
        let Some(raster) = current.clone().use_render_image(window, cx) else {
            return;
        };
        if let Some(displayed) = self.displayed.take() {
            // The tile is already superseded on screen; a failed drop only
            // leaks what the next retire would have freed anyway.
            let _ = window.drop_image(displayed.raster);
            displayed.source.remove_asset(cx);
        }
        self.displayed = Some(DisplayedImage {
            source: current,
            raster,
        });
    }

    fn cycle(&mut self, next: impl FnOnce(&mut Self), cx: &mut Context<Self>) {
        next(self);
        self.request(cx);
        cx.notify();
    }

    fn next_fixture(&mut self, _: &NextFixture, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.fixtures.len();
        self.window_scale = window.scale_factor();
        self.cycle(|view| view.fixture_idx = (view.fixture_idx + 1) % count, cx);
    }

    fn next_viewport(&mut self, _: &NextViewport, window: &mut Window, cx: &mut Context<Self>) {
        self.window_scale = window.scale_factor();
        self.cycle(
            |view| view.viewport_idx = (view.viewport_idx + 1) % VIEWPORTS.len(),
            cx,
        );
    }

    fn next_scale(&mut self, _: &NextScale, window: &mut Window, cx: &mut Context<Self>) {
        self.window_scale = window.scale_factor();
        self.cycle(
            |view| view.scale_idx = (view.scale_idx + 1) % SCALES.len(),
            cx,
        );
    }

    fn next_density(&mut self, _: &NextDensity, window: &mut Window, cx: &mut Context<Self>) {
        self.window_scale = window.scale_factor();
        self.cycle(
            |view| view.density_idx = (view.density_idx + 1) % DENSITIES.len(),
            cx,
        );
    }

    fn status(&self) -> String {
        let (name, content) = &self.fixtures[self.fixture_idx];
        let (_, _, viewport) = VIEWPORTS[self.viewport_idx];
        format!(
            "{name} ({:?}) · {viewport} · {}% · {}% · {}/{}",
            content.kind,
            SCALES[self.scale_idx].percent(),
            DENSITIES[self.density_idx],
            self.fixture_idx + 1,
            self.fixtures.len(),
        )
    }

    fn diagnostics_panel(&self, environment: UiEnvironment) -> impl IntoElement {
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let mut lines = match &self.rendered {
            None => vec!["composing…".to_owned()],
            Some(rendered) => {
                let (pixel_w, pixel_h) = rendered.frame.pixel_size();
                let (logical_w, logical_h) = rendered.frame.logical_size();
                let mut lines = vec![
                    format!("raster {pixel_w}x{pixel_h} at {}%", rendered.key.1),
                    format!("logical {logical_w}x{logical_h}"),
                ];
                if rendered.diagnostics.warnings.is_empty() {
                    lines.push("no warnings".to_owned());
                } else {
                    lines.extend(
                        rendered
                            .diagnostics
                            .warnings
                            .iter()
                            .map(|warning| format!("warning: {warning}")),
                    );
                }
                lines
            }
        };
        let body = std::mem::take(&mut lines).join("\n");
        div()
            .flex_none()
            .w(px(scale.apply(360.0)))
            .p(px(scale.apply(tokens::spacing::GROUP)))
            .text_size(px(scale.apply(tokens::typography::BODY.size)))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                    .child(self.status()),
            )
            .child(
                div()
                    .pt(px(scale.apply(tokens::spacing::RELATED)))
                    .child(body),
            )
            .child(
                div()
                    .pt(px(scale.apply(tokens::spacing::GROUP)))
                    .text_size(px(scale.apply(tokens::typography::BODY.size)))
                    .text_color(rgb(palette.text_muted))
                    .child("F2 fixture · F3 viewport · F4 scale · F5 density"),
            )
    }
}

impl Focusable for LabView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for LabView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = self.environment();
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let (preset_w, preset_h, _) = VIEWPORTS[self.viewport_idx];
        let current = self
            .rendered
            .as_ref()
            .map(|rendered| rendered.frame.image().clone());
        self.retire_outgoing_upload(current, window, cx);
        let frame = self.rendered.as_ref().map(|rendered| &rendered.frame);

        div()
            .id("canonical-slide-lab")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .on_action(cx.listener(Self::next_fixture))
            .on_action(cx.listener(Self::next_viewport))
            .on_action(cx.listener(Self::next_scale))
            .on_action(cx.listener(Self::next_density))
            .child(
                div()
                    .flex_1()
                    .id("canonical-lab-stage")
                    .overflow_scroll()
                    .child(
                        div()
                            .flex_none()
                            .w(px(preset_w))
                            .h(px(preset_h))
                            .p(px(scale.apply(tokens::spacing::SECTION)))
                            .flex()
                            .flex_row()
                            .gap(px(scale.apply(tokens::spacing::SECTION)))
                            .children(
                                frame.map(|frame| {
                                    canonical_views::preview_frame(frame, environment)
                                }),
                            )
                            .child(self.diagnostics_panel(environment)),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .gap(px(scale.apply(tokens::spacing::GROUP)))
                    .p(px(scale.apply(tokens::spacing::GROUP)))
                    .border_t(px(tokens::border::SEPARATOR))
                    .border_color(rgb(palette.border))
                    .children(frame.map(|frame| {
                        canonical_views::timeline_thumbnail(frame, self.fixture_idx, environment)
                    }))
                    .children(
                        frame.map(|frame| canonical_views::inspector_frame(frame, environment)),
                    )
                    .children(
                        frame.map(|frame| canonical_views::program_frame(frame, environment)),
                    ),
            )
    }
}

fn main() {
    use platform::tasks::{OwnedWorker, WorkerKind};

    let fixtures = load_fixtures();
    // The production canonical worker, owned exactly as the packaged
    // runtime owns it: composition runs on the `ordo-canonical` thread
    // behind `CanonicalCommand`, never on the GPUI thread.
    let (canonical_tx, canonical_rx) = std::sync::mpsc::channel();
    let canonical = OwnedWorker::spawn(WorkerKind::Canonical, move |cancel| {
        ordo::canonical::serve_canonical(canonical_rx, cancel)
    })
    .expect("canonical worker must start");
    let (requests, receiver) = async_channel::unbounded();
    spawn_worker(receiver, canonical_tx.clone());
    let stop_canonical = canonical_tx;
    application().run(move |cx: &mut App| {
        ui::init(cx).expect("the UI environment should initialize");
        cx.bind_keys([
            KeyBinding::new("f2", NextFixture, None),
            KeyBinding::new("f3", NextViewport, None),
            KeyBinding::new("f4", NextScale, None),
            KeyBinding::new("f5", NextDensity, None),
        ]);
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        let fixtures = fixtures.clone();
        let requests = requests.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let lab = LabView::new(fixtures, requests, window, cx);
                    window.focus(&lab.focus_handle(cx), cx);
                    lab
                })
            },
        )
        .expect("the canonical slide lab window should open");
        cx.activate(true);
    });
    // The app has quit: stop the canonical worker before joining it. (If
    // the loop above never returns, the process exit ends both threads.)
    stop_canonical
        .send(ordo::canonical::CanonicalCommand::Stop)
        .ok();
    drop(canonical);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    /// Gallery capture through the live lab (T0.4 PR 10).
    ///
    /// Every candidate fixture drives the actual `LabView` (`open` plus its
    /// `spawn_worker` raster thread): the lab's `load_fixtures` order, the
    /// production canonical worker behind `CanonicalCommand::Compose`, the
    /// 100% raster with the solid-gray GeneralImage stand-in, and the
    /// `canonical_views::finish_frame` handoff. The collected
    /// `Rendered.frame` plus `diagnostics` are those exact live-lab results,
    /// not a re-run of the pipeline beside the lab. These assertions always
    /// run, so the whole corpus stays composable through the lab path.
    /// Setting `ORDO_GALLERY_CAPTURE` to an output directory additionally
    /// writes `<name>.png` (finished pre-chrome slide pixels) plus
    /// `capture-rows.md`: the visible frame, the wrapper, and the
    /// diagnostics riding beside it, which is the exact source of the
    /// candidate matrix in `docs/qualification/T0.4-GALLERY-CANDIDATE.md`.
    /// The PNGs are slide pixels paired with wrapper/diagnostics metadata —
    /// never a compositor screenshot (`TestAppContext` supplies no headless
    /// renderer on any OS, so none is attempted here). The captures stay
    /// candidate review artifacts;
    /// they never promote themselves to `design-references/`.
    #[gpui::test]
    async fn gallery_capture_through_the_lab_pipeline(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        cx.condition(&lab, |lab, _| lab.rendered.is_some()).await;
        let count = lab.read_with(cx, |lab, _| lab.fixtures.len());

        let mut rows = Vec::new();
        let mut pngs = Vec::new();
        for expected_idx in 0..count {
            cx.condition(&lab, |lab, _| {
                lab.rendered.as_ref().is_some_and(|rendered| {
                    rendered.sequence == lab.sequence
                        && rendered.key == (expected_idx, DENSITIES[0])
                })
            })
            .await;
            let (
                name,
                kind,
                header,
                authored,
                body_rendered,
                footer_rendered,
                warnings,
                png_bytes,
                logical,
                pixel,
            ) = lab.read_with(cx, |lab, _| {
                assert_eq!(
                    lab.fixture_idx, expected_idx,
                    "the live lab must be showing fixture {expected_idx}"
                );
                let (name, content) = &lab.fixtures[expected_idx];
                let rendered = lab
                    .rendered
                    .as_ref()
                    .expect("the live lab must have rendered");
                assert_eq!(
                    rendered.key,
                    (expected_idx, DENSITIES[0]),
                    "the live lab must render fixture {expected_idx} at 100%"
                );
                let cell = |text: &str| text.replace('|', "/");
                let header = match &content.header {
                    None => "absent".to_owned(),
                    Some(header) if header.is_empty() => "empty".to_owned(),
                    Some(header) => format!("`{}`", cell(header)),
                };
                let authored = if content.body.is_empty() {
                    0
                } else {
                    content.body.lines().count()
                };
                let warnings = if rendered.diagnostics.warnings.is_empty() {
                    "none".to_owned()
                } else {
                    rendered
                        .diagnostics
                        .warnings
                        .iter()
                        .map(|warning| cell(warning))
                        .collect::<Vec<_>>()
                        .join("<br>")
                };
                (
                    name.clone(),
                    format!("{:?}", content.kind),
                    header,
                    authored,
                    rendered.body_lines,
                    rendered.footer_lines,
                    warnings,
                    rendered.frame.image().bytes().to_vec(),
                    rendered.frame.logical_size(),
                    rendered.frame.pixel_size(),
                )
            });
            assert_eq!(
                logical,
                (1920, 1080),
                "{name} must compose in 1920x1080 logical coordinates"
            );
            assert_eq!(
                pixel,
                (1920, 1080),
                "{name} must rasterize 1920x1080 at 100% density"
            );
            rows.push(format!(
                "| {name} | {kind} | {header} | {authored}/{body_rendered} | {footer_rendered} | {warnings} | `gallery-candidate/{name}.png` |",
            ));
            pngs.push((name, png_bytes));
            if expected_idx + 1 < count {
                cx.simulate_keystrokes("f2");
                cx.run_until_parked();
            }
        }

        assert_eq!(
            rows.len(),
            count,
            "every fixture must capture exactly one row"
        );
        let Some(dir) = std::env::var_os("ORDO_GALLERY_CAPTURE") else {
            return;
        };
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|error| {
            panic!("capture dir {} must be writable: {error}", dir.display())
        });
        for (name, bytes) in &pngs {
            std::fs::write(dir.join(format!("{name}.png")), bytes)
                .unwrap_or_else(|error| panic!("capture {name}.png must be writable: {error}"));
        }
        let mut manifest = String::from(
            "# Gallery capture provenance\n\
             \n\
             - Pipeline: the live `LabView` (`open` plus its `spawn_worker` raster thread): \
             `load_fixtures` order → production canonical worker (`CanonicalCommand::Compose`, \
             default Theme, 100% density) → `rendering::rasterize` at 100% (solid-gray stand-in \
             for GeneralImage foregrounds) → `ordo::canonical_views::finish_frame`. Each row's \
             `Rendered.frame` plus `diagnostics` below are those exact live-lab results.\n\
             - Visible frame: finished pre-chrome slide pixels, `1920x1080` logical, `1920x1080` \
             raster at 100% density — slide pixels paired with wrapper/diagnostics metadata, not \
             a compositor screenshot (`TestAppContext` supplies no headless renderer on any OS, \
             so none is attempted here).\n\
             - Wrapper: `ui::canonical_frame::canonical_frame` through `ordo::canonical_views` \
             (`timeline_thumbnail` / `inspector_frame` / `preview_frame` / `program_frame`, 320 / 480 / \
             720 / 960 before Interface scale, `4px` operator-only chrome); the congregation surface \
             reserves the same sampling edge-to-edge with square corners (black until its background \
             encode lands).\n\
             - Diagnostics: the Warnings column below rides beside each capture; overflow fixtures warn \
             beside complete frames.\n\
             - Live review: `cargo run -p ordo --example canonical-slide-lab` (`F2` fixture · `F3` \
             viewport · `F4` scale · `F5` density; diagnostics beside the scaled frame).\n\
             - Status: candidate for review only — not in `design-references/`, never an Approved \
             Reference until the live output and its captured reference are approved together in \
             `T0.4-MANUAL-GATE.md`.\n\
             \n\
             | Fixture | Template | Header | Body lines (authored/rendered) | Footer lines (rendered) | Warnings | PNG |\n\
             | --- | --- | --- | --- | --- | --- | --- |\n",
        );
        for row in &rows {
            manifest.push_str(row);
            manifest.push('\n');
        }
        std::fs::write(dir.join("capture-rows.md"), manifest)
            .expect("capture rows must be writable");
    }

    fn open(cx: &mut TestAppContext) -> (gpui::Entity<LabView>, &mut gpui::VisualTestContext) {
        // The composition worker is a real OS thread, so the test executor
        // must park for its replies the way the runtime tests do.
        cx.executor().allow_parking();
        cx.update(ui::init).unwrap();
        cx.update(|cx| {
            cx.bind_keys([
                KeyBinding::new("f2", NextFixture, None),
                KeyBinding::new("f3", NextViewport, None),
                KeyBinding::new("f4", NextScale, None),
                KeyBinding::new("f5", NextDensity, None),
            ]);
        });
        let fixtures = load_fixtures();
        let (requests, receiver) = async_channel::unbounded();
        // The production canonical worker, exactly as the packaged runtime
        // owns it. The handle is leaked on purpose: joining it here could
        // hang teardown (the raster thread holds a sender until the view
        // dies), while channel close always ends the thread once every
        // sender is gone.
        let (canonical_tx, canonical_rx) = std::sync::mpsc::channel();
        let canonical = platform::tasks::OwnedWorker::spawn(
            platform::tasks::WorkerKind::Canonical,
            move |cancel| ordo::canonical::serve_canonical(canonical_rx, cancel),
        )
        .expect("canonical worker must start");
        let _leaked: &'static _ = Box::leak(Box::new(canonical));
        spawn_worker(receiver, canonical_tx);
        let (lab, cx) = cx.add_window_view(|window, cx| {
            let lab = LabView::new(fixtures, requests, window, cx);
            window.focus(&lab.focus_handle, cx);
            lab
        });
        cx.simulate_resize(size(px(1400.0), px(900.0)));
        (lab, cx)
    }

    #[gpui::test]
    async fn lab_keys_cycle_fixture_viewport_scale_and_density(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        cx.condition(&lab, |lab, _| lab.rendered.is_some()).await;

        cx.simulate_keystrokes("f2");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| assert_eq!(lab.fixture_idx, 1));

        cx.simulate_keystrokes("f3");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| assert_eq!(lab.viewport_idx, 1));

        cx.simulate_keystrokes("f4");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| assert_eq!(lab.scale_idx, 1));

        cx.simulate_keystrokes("f5");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| assert_eq!(lab.density_idx, 1));

        // The worker round-trips the latest selection: the stale guard must
        // not drop it.
        cx.condition(&lab, |lab, _| {
            lab.rendered
                .as_ref()
                .is_some_and(|rendered| rendered.sequence == lab.sequence)
        })
        .await;

        lab.read_with(cx, |lab, _| {
            let rendered = lab.rendered.as_ref().expect("latest selection must render");
            assert_eq!(rendered.key.1, DENSITIES[1]);
        });

        // A density change followed by a fixture change must still render
        // the new fixture: the stale-sequence guard must drop the late
        // density reply without wedging later selections. Each wait covers
        // one render; chaining both behind a single budget would flake
        // against the fixed condition timeout on slow renders.
        cx.simulate_keystrokes("f5 f2");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| assert_eq!(lab.fixture_idx, 2));
        cx.condition(&lab, |lab, _| lab.cache.contains_key(&(1, DENSITIES[1])))
            .await;
        cx.condition(&lab, |lab, _| {
            lab.rendered
                .as_ref()
                .is_some_and(|rendered| rendered.sequence == lab.sequence)
        })
        .await;

        // Mashing selectors must converge on the latest one: the worker
        // coalesces to the newest pending request instead of queueing a
        // seconds-long render per keypress.
        cx.simulate_keystrokes("f5 f5 f5 f2");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| {
            assert_eq!(lab.fixture_idx, 3);
            assert_eq!(lab.density_idx, 0);
        });
        cx.condition(&lab, |lab, _| {
            lab.rendered
                .as_ref()
                .is_some_and(|rendered| rendered.sequence == lab.sequence)
        })
        .await;

        lab.read_with(cx, |lab, _| {
            let rendered = lab.rendered.as_ref().expect("latest selection must render");
            assert_eq!(rendered.key.1, DENSITIES[0]);
        });

        // Chrome-only changes keep the same pixels: viewport and scale
        // selectors display synchronously instead of re-rendering. The
        // sequence still advances, so a late in-flight reply can never
        // collide with a later request.
        let sequence = lab.read_with(cx, |lab, _| lab.sequence);
        cx.simulate_keystrokes("f3");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| {
            assert_eq!(lab.sequence, sequence + 1);
            assert_eq!(
                lab.rendered.as_ref().map(|rendered| rendered.key),
                Some((3, DENSITIES[0])),
                "same pixels must display without a worker round-trip"
            );
        });
        cx.simulate_keystrokes("f4");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| {
            assert_eq!(lab.sequence, sequence + 2);
            assert_eq!(
                lab.rendered.as_ref().map(|rendered| rendered.key),
                Some((3, DENSITIES[0]))
            );
        });

        // Eviction: the outgoing slide leaves the asset cache and the
        // atlas once the incoming one decodes, instead of accumulating a
        // decoded copy plus tile per visited variant.
        let outgoing = lab.read_with(cx, |lab, _| {
            lab.rendered.as_ref().unwrap().frame.image().clone()
        });
        let outgoing_raster = cx
            .update(|window, cx| outgoing.clone().get_render_image(window, cx))
            .expect("the displayed slide must be decoded");
        cx.simulate_keystrokes("f2");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| assert_eq!(lab.fixture_idx, 4));
        cx.condition(&lab, |lab, _| {
            lab.rendered
                .as_ref()
                .is_some_and(|rendered| rendered.sequence == lab.sequence)
        })
        .await;
        // Frames render on demand under test (unlike the live loop), and
        // the decode completion itself only schedules an on-next-frame
        // notify, so deliver those scheduled frames until the post-decode
        // retire runs: each pass advances the background decode and then
        // renders against it. No explicit notify is issued here on purpose:
        // the retire path must schedule its own re-render through
        // `use_render_image`, like the viewports' `img` elements. A `get`
        // would create the asset task without a waiter, stealing `is_first`
        // from the same frame's `use_asset` calls, so the decode would
        // finish with nothing scheduled and the viewport would stay black.
        let mut scheduled_any = false;
        for _ in 0..100 {
            cx.run_until_parked();
            let delivered = cx.update(|window, cx| window.simulate_next_frame(cx));
            scheduled_any |= delivered > 0;
            cx.run_until_parked();
            let swapped = lab.read_with(cx, |lab, _| {
                lab.displayed
                    .as_ref()
                    .is_some_and(|displayed| displayed.source.id() != outgoing.id())
            });
            if swapped {
                break;
            }
        }
        assert!(
            scheduled_any,
            "the incoming decode must schedule its own re-render instead of staying black"
        );
        lab.read_with(cx, |lab, _| {
            assert!(
                lab.displayed
                    .as_ref()
                    .is_some_and(|displayed| displayed.source.id() != outgoing.id()),
                "the incoming slide must take the screen once decoded"
            );
        });
        cx.run_until_parked();
        assert!(
            cx.update(|_, cx| !gpui::ImageSource::Image(outgoing.clone()).is_asset_cached(cx)),
            "the retired slide must leave the asset cache"
        );
        assert!(
            cx.update(|window, _| !window.has_image_atlas_entry(&outgoing_raster)),
            "the retired slide must leave the atlas"
        );
    }
}
