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

use std::{collections::HashMap, path::PathBuf, thread};

use async_channel::{Receiver, Sender};
use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, Focusable, IntoElement, KeyBinding,
    ParentElement, Render, Styled, Window, WindowBounds, WindowOptions, actions, div, prelude::*,
    px, rgb, size,
};
use gpui_platform::application;
use ordo::canonical_views;
use rendering::{Diagnostics, TargetDensity, TemplateContent, TemplateKind, Theme};
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
    content: TemplateContent,
    density_percent: u16,
    reply: Sender<Rendered>,
}

struct Rendered {
    sequence: u64,
    frame: FinishedFrame,
    diagnostics: Diagnostics,
    density_percent: u16,
}

/// The lab's composition worker: the real `render_template` plus `rasterize`
/// path on a background thread, so rendering never blocks the GPUI thread.
/// Density reaches only the raster extent; the logical composition is
/// density-independent by construction.
fn spawn_worker(requests: Receiver<Request>) {
    thread::spawn(move || {
        while let Ok(request) = requests.recv_blocking() {
            let density = TargetDensity::from_percent(request.density_percent)
                .unwrap_or(TargetDensity::SCALE_100);
            let rendered = match rendering::render_template(
                &request.content,
                &Theme::default(),
                TargetDensity::SCALE_100,
            ) {
                Ok((frame, diagnostics)) => {
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
                                frame,
                                diagnostics,
                                density_percent: request.density_percent,
                            })
                        }
                        Err(error) => {
                            eprintln!("lab raster failed: {error}");
                            None
                        }
                    }
                }
                Err(error) => {
                    eprintln!("lab composition failed: {error}");
                    None
                }
            };
            if let Some(rendered) = rendered
                && request.reply.send_blocking(rendered).is_err()
            {
                break;
            }
        }
    });
}

struct LabView {
    fixtures: Vec<(String, TemplateContent)>,
    fixture_idx: usize,
    viewport_idx: usize,
    scale_idx: usize,
    density_idx: usize,
    sequence: u64,
    rendered: Option<Rendered>,
    requests: Sender<Request>,
    focus_handle: FocusHandle,
}

impl LabView {
    fn new(
        fixtures: Vec<(String, TemplateContent)>,
        requests: Sender<Request>,
        _: &mut Window,
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

    /// Send the current selection to the worker. Replies carry the request
    /// sequence, so a stale composition that finishes late cannot overwrite a
    /// newer one.
    fn request(&mut self, cx: &mut Context<Self>) {
        self.sequence += 1;
        let sequence = self.sequence;
        let content = self.fixtures[self.fixture_idx].1.clone();
        let density_percent = DENSITIES[self.density_idx];
        let (reply_tx, reply_rx) = async_channel::bounded(1);
        let requests = self.requests.clone();
        cx.spawn(async move |view, cx| {
            if requests
                .send(Request {
                    sequence,
                    content,
                    density_percent,
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
                if rendered.sequence == view.sequence {
                    view.rendered = Some(rendered);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn cycle(&mut self, next: impl FnOnce(&mut Self), cx: &mut Context<Self>) {
        next(self);
        self.request(cx);
        cx.notify();
    }

    fn next_fixture(&mut self, _: &NextFixture, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.fixtures.len();
        self.cycle(|view| view.fixture_idx = (view.fixture_idx + 1) % count, cx);
    }

    fn next_viewport(&mut self, _: &NextViewport, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle(
            |view| view.viewport_idx = (view.viewport_idx + 1) % VIEWPORTS.len(),
            cx,
        );
    }

    fn next_scale(&mut self, _: &NextScale, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle(
            |view| view.scale_idx = (view.scale_idx + 1) % SCALES.len(),
            cx,
        );
    }

    fn next_density(&mut self, _: &NextDensity, _: &mut Window, cx: &mut Context<Self>) {
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
                    format!(
                        "raster {pixel_w}x{pixel_h} at {}%",
                        rendered.density_percent
                    ),
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
            .child(self.status())
            .child(
                div()
                    .pt(px(scale.apply(tokens::spacing::RELATED)))
                    .child(body),
            )
            .child(
                div()
                    .pt(px(scale.apply(tokens::spacing::GROUP)))
                    .text_size(px(scale.apply(tokens::typography::LABEL.size)))
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = self.environment();
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let (preset_w, preset_h, _) = VIEWPORTS[self.viewport_idx];
        let frame = self.rendered.as_ref().map(|rendered| &rendered.frame);

        div()
            .id("canonical-slide-lab")
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
    let fixtures = load_fixtures();
    let (requests, receiver) = async_channel::unbounded();
    spawn_worker(receiver);
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
}
