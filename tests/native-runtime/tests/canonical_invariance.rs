//! T0.4 PR 8: packaged cross-platform invariance.
//!
//! One mixed-language probe through the real `rendering` path must serialize
//! byte-identically across all four axes: packaged platform (Windows/Linux),
//! Ordo Interface scale (`100%`–`200%`), representative system scale, and
//! every output density. Each axis failure names the axis that moved.
//!
//! This runs where the packaged binary runs: the `native-runtime` harness
//! builds the `ordo` closure (`ordo` + `rendering` + `ui` together) rather
//! than testing `rendering` alone. Scale bytes come out of the worker-owned
//! production path — live operator state snapshotted through production
//! plumbing, composed on the runtime-owned `ordo-canonical` worker thread,
//! never on the GPUI thread. The platform axis compares against the
//! committed `golden/canonical-invariance.txt`, which both CI targets check;
//! a platform-dependent composition re-opens
//! `docs/qualification/RENDERING-T0.4.md` instead of updating that artifact.
//! The artifact regenerates only with `ORDO_UPDATE_CANONICAL_INVARIANCE=1`,
//! and only when the new composition is reviewed as correct.
//!
//! No claim about pixels here: density may change raster dimensions and
//! sharpness, and per-platform sharpness is qualified separately (PR 10).
//! Only the serialized logical composition is pinned.

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{Pixels, TestAppContext};
use ordo::{NativeRuntime, canonical::CanonicalCommand, open_native_runtime_at};
use rendering::{Diagnostics, Frame, TargetDensity, TemplateContent, TemplateKind, Theme};
use ui::{DecreaseInterfaceScale, IncreaseInterfaceScale};

/// The probe: Cebuano/English/Tagalog Latin plus digits, punctuation, an
/// em-dash, and fallback glyphs (Greek alpha, peso sign) that the bundled
/// Noto face carries with real ids — see `rendering`'s `tests/fonts.rs`.
/// Never `.notdef` and never an installed font.
fn probe_content() -> TemplateContent {
    TemplateContent {
        kind: TemplateKind::Lyrics,
        header: Some("Mixed".to_owned()),
        body: "Maayong buntag, mga igsoon!\nmagandang umaga, mga kapatid \u{2014} awit 95 \u{3B1}\nthe choir sings, salmo 7 \u{20B1}"
            .to_owned(),
        footer: None,
        image: None,
    }
}

/// The pinned artifact shape for one completed frame: the serialized logical
/// frame plus the diagnostics riding beside it, the same shape `rendering`'s
/// golden harness pins per fixture. The worker returns the payload; pins and
/// handoff serialize it at the edge, exactly as PR 9's consumers will take
/// the frame itself.
fn format_probe(frame: &Frame, diagnostics: &Diagnostics) -> String {
    let mut out = frame.serialize();
    out.push_str(&format!("warnings {}\n", diagnostics.warnings.len()));
    for warning in &diagnostics.warnings {
        out.push_str(&format!("warning {warning}\n"));
    }
    out
}

/// The pinned artifact for the probe: the serialized logical frame plus the
/// diagnostics riding beside it, the same shape `rendering`'s golden harness
/// pins per fixture.
fn probe_serialization(density: TargetDensity) -> String {
    let (frame, diagnostics) =
        rendering::render_template(&probe_content(), &Theme::default(), density)
            .expect("invariance probe must compose");
    format_probe(&frame, &diagnostics)
}

/// Bytes out of the worker-owned production path: snapshot live operator
/// state through production plumbing, send one composition request on the
/// runtime-owned channel, and receive the completed frame plus diagnostics
/// the `ordo-canonical` worker composed off the GPUI thread. Every scale
/// byte under test flows through this call; the pin serializes the payload
/// here at the edge.
fn compose_through_worker(
    cx: &mut TestAppContext,
    runtime: &NativeRuntime,
) -> (Frame, Diagnostics) {
    let (environment, window_scale) = runtime
        .operator()
        .update(cx, |root, window, _| {
            (root.environment(), window.scale_factor())
        })
        .expect("operator window should still be open");
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    runtime
        .canonical_commands()
        .send(CanonicalCommand::Compose {
            environment,
            window_scale,
            content: probe_content(),
            reply: reply_tx,
        })
        .expect("canonical worker should still be running");
    reply_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("canonical worker must answer")
        .expect("probe must compose through the runtime path")
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/canonical-invariance.txt")
}

/// The first differing line — which names the element and coordinates that
/// moved — instead of reporting that two serializations differ.
fn first_mismatch(actual: &str, expected: &str) -> String {
    let actual_lines: Vec<&str> = actual.lines().collect();
    let expected_lines: Vec<&str> = expected.lines().collect();
    if let Some(index) = actual_lines
        .iter()
        .zip(expected_lines.iter())
        .position(|(first, second)| first != second)
    {
        return format!(
            "line {}:\n  expected: {}\n  actual:   {}",
            index + 1,
            expected_lines[index],
            actual_lines[index]
        );
    }
    let shared = actual_lines.len().min(expected_lines.len());
    let (side, line) = if actual_lines.len() > expected_lines.len() {
        ("actual-only", actual_lines[shared])
    } else {
        ("expected-only", expected_lines[shared])
    };
    format!(
        "same first {shared} lines but different length (expected {} lines, actual {} lines); first {side} line {}: {line}",
        expected_lines.len(),
        actual_lines.len(),
        shared + 1
    )
}

#[test]
fn canonical_composition_is_invariant_across_density_and_platform() {
    let baseline = probe_serialization(TargetDensity::SCALE_100);
    // The probe carries fallback glyphs, so a platform shaper or fallback
    // regression changes the pinned bytes rather than passing quietly — and
    // every one of them is a real glyph, never `.notdef`.
    assert!(
        baseline.contains("font=fallback"),
        "probe must exercise the bundled fallback path"
    );
    assert!(
        !baseline.lines().any(|line| line.contains(" id=0 ")),
        "probe must pin real fallback glyphs, never `.notdef`"
    );

    // Output density: every qualified step must rerasterize without recomposing.
    for percent in [100, 125, 150, 175, 200] {
        let density = TargetDensity::from_percent(percent).expect("valid density");
        let actual = probe_serialization(density);
        assert!(
            actual == baseline,
            "canonical composition moved with output density {percent}%: {}\nTargetDensity must change raster dimensions and sharpness only",
            first_mismatch(&actual, &baseline)
        );
    }

    // Density reaches the raster extent only: 100% samples the 1920x1080
    // logical composition 1:1, 200% samples the same composition at 2x.
    let (frame, _) = rendering::render_template(
        &probe_content(),
        &Theme::default(),
        TargetDensity::SCALE_100,
    )
    .expect("invariance probe must compose");
    let images = HashMap::new();
    let light = rendering::rasterize(&frame, TargetDensity::SCALE_100, &images)
        .expect("probe must rasterize");
    let dense = rendering::rasterize(
        &frame,
        TargetDensity::from_percent(200).expect("valid density"),
        &images,
    )
    .expect("probe must rasterize");
    assert_eq!(
        (light.width(), light.height()),
        (1920, 1080),
        "100% raster extent must stay the logical extent"
    );
    assert_eq!(
        (dense.width(), dense.height()),
        (3840, 2160),
        "200% raster extent must double the logical extent"
    );

    // Packaged platform: both CI targets compare their run against this
    // committed artifact. A mismatch names the platform, not a diff of blobs.
    let golden = golden_path();
    if std::env::var_os("ORDO_UPDATE_CANONICAL_INVARIANCE").is_some() {
        std::fs::create_dir_all(golden.parent().expect("golden must have a parent"))
            .expect("golden dir must be writable");
        std::fs::write(&golden, &baseline).expect("golden must be writable");
        return;
    }
    let expected = std::fs::read_to_string(&golden).unwrap_or_else(|_| {
        panic!(
            "missing invariance golden at {}: run with ORDO_UPDATE_CANONICAL_INVARIANCE=1 to create it, then review the diff",
            golden.display()
        )
    });
    assert!(
        baseline == expected,
        "packaged {} serialization differs from {}: {}\nA platform-dependent composition re-opens docs/qualification/RENDERING-T0.4.md instead of updating this artifact",
        std::env::consts::OS,
        golden.display(),
        first_mismatch(&baseline, &expected)
    );
}

/// Operator chrome geometry from live runtime state, through the same token
/// path the operator UI renders with. This demonstrably moves when operator
/// scale does, so the isolation test below cannot pass with scale stuck.
fn operator_shadow_offset_y(cx: &mut TestAppContext, runtime: &NativeRuntime) -> Pixels {
    let environment = runtime
        .operator()
        .update(cx, |root, _, _| root.environment())
        .expect("operator window should still be open");
    ui::tokens::elevation::floating(environment.appearance(), environment.preferences().scale)
        .offset
        .y
}

#[gpui::test]
fn operator_scale_cannot_reach_the_canonical_frame(cx: &mut TestAppContext) {
    // T0.3 PR 3 deferred this isolation proof for want of a render path. It
    // lands here, in the packaged runtime against real 1920x1080 geometry,
    // where it can fail: every byte under test is composed by the
    // runtime-owned worker from live operator state after each real scale
    // action — while operator chrome geometry demonstrably moves with each
    // step.
    let preferences =
        std::env::temp_dir().join(format!("ordo-canonical-invariance-{}", std::process::id()));
    let runtime = cx
        .update(|cx| open_native_runtime_at(cx, false, &preferences).expect("runtime should open"));
    let (baseline_frame, baseline_diagnostics) = compose_through_worker(cx, &runtime);
    let baseline = format_probe(&baseline_frame, &baseline_diagnostics);
    assert_eq!(
        baseline,
        probe_serialization(TargetDensity::SCALE_100),
        "the worker path must agree with the pinned composition"
    );
    let mut previous_offset = operator_shadow_offset_y(cx, &runtime);

    for expected in [125, 150, 175, 200] {
        cx.dispatch_action(runtime.operator().into(), IncreaseInterfaceScale);
        cx.run_until_parked();
        let scale = runtime
            .operator()
            .update(cx, |root, _, _| root.environment().preferences().scale)
            .expect("operator window should still be open");
        assert_eq!(scale.percent(), expected);
        let offset = operator_shadow_offset_y(cx, &runtime);
        assert!(
            offset != previous_offset,
            "operator chrome must move with live operator scale {expected}%"
        );
        previous_offset = offset;
        let (actual_frame, actual_diagnostics) = compose_through_worker(cx, &runtime);
        let actual = format_probe(&actual_frame, &actual_diagnostics);
        assert!(
            actual == baseline,
            "canonical composition moved with live operator scale {expected}%: {}",
            first_mismatch(&actual, &baseline)
        );
    }
    for expected in [175, 150, 125, 100] {
        cx.dispatch_action(runtime.operator().into(), DecreaseInterfaceScale);
        cx.run_until_parked();
        let scale = runtime
            .operator()
            .update(cx, |root, _, _| root.environment().preferences().scale)
            .expect("operator window should still be open");
        assert_eq!(scale.percent(), expected);
        let offset = operator_shadow_offset_y(cx, &runtime);
        assert!(
            offset != previous_offset,
            "operator chrome must move with live operator scale {expected}%"
        );
        previous_offset = offset;
        let (actual_frame, actual_diagnostics) = compose_through_worker(cx, &runtime);
        let actual = format_probe(&actual_frame, &actual_diagnostics);
        assert!(
            actual == baseline,
            "canonical composition moved with live operator scale {expected}%: {}",
            first_mismatch(&actual, &baseline)
        );
    }

    runtime
        .operator()
        .update(cx, |_, window, _| window.remove_window())
        .expect("operator window should still be open");
    cx.run_until_parked();
    std::fs::remove_file(preferences).ok();
}

#[gpui::test]
fn display_scale_cannot_reach_the_canonical_frame(cx: &mut TestAppContext) {
    // Representative Windows display-scale and Linux fractional-scale steps.
    // Each step reconfigures the packaged operator window through gpui's own
    // display scale seam; the worker then composes from that window's live
    // scale, so the renderer sees distinct host configurations — not one
    // unchanged setting — and the composition must not move under any of
    // them.
    let preferences = std::env::temp_dir().join(format!(
        "ordo-canonical-display-scale-{}",
        std::process::id()
    ));
    let runtime = cx
        .update(|cx| open_native_runtime_at(cx, false, &preferences).expect("runtime should open"));
    let (baseline_frame, baseline_diagnostics) = compose_through_worker(cx, &runtime);
    let baseline = format_probe(&baseline_frame, &baseline_diagnostics);
    assert_eq!(
        baseline,
        probe_serialization(TargetDensity::SCALE_100),
        "the worker path must agree with the pinned composition"
    );

    for factor in [1.0f32, 1.25, 1.5, 1.75, 2.0] {
        runtime
            .operator()
            .update(cx, |_, window, _| window.set_scale_factor(factor))
            .expect("operator window should still be open");
        cx.run_until_parked();
        let (actual_frame, actual_diagnostics) = compose_through_worker(cx, &runtime);
        let actual = format_probe(&actual_frame, &actual_diagnostics);
        let observed = runtime
            .operator()
            .update(cx, |_, window, _| window.scale_factor())
            .expect("operator window should still be open");
        assert_eq!(
            observed, factor,
            "display-scale configuration {factor} must take effect on the packaged window"
        );
        assert!(
            actual == baseline,
            "canonical composition moved with system scale {}%: {}\nWindow scale must never reach the renderer",
            (factor * 100.0) as u16,
            first_mismatch(&actual, &baseline)
        );
    }

    runtime
        .operator()
        .update(cx, |_, window, _| window.remove_window())
        .expect("operator window should still be open");
    cx.run_until_parked();
    std::fs::remove_file(preferences).ok();
}
