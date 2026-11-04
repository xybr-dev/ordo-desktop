//! Presenter live-control E2E (T1.4 WP8).
//!
//! The harness is `super`'s (`open`, `run`, `bounds`, ...). It lives inside
//! the crate because the Presenter's frame store and view fields are only
//! reachable there; an integration test would need extra public accessors.
//!
//! # `[UI-AUTO]` proof map
//!
//! Proofs 1 to 7 already have focused tests in `super::tests`; the E2E below
//! re-asserts 6 and 7 together at every step.
//!
//! 1. Wide layout: `wide_places_the_rail_timeline_and_dock_in_order`
//! 2. Compact and Constrained:
//!    `compact_uses_the_tab_and_a_360_dock_with_program_above_preview`,
//!    `constrained_puts_both_monitors_in_a_band_above_the_timeline`,
//!    `constrained_overlay_has_no_pin_and_a_scrim_press_closes_it_without_going_through`
//! 3. Grid/List: `grid_list_saves_the_preference_and_never_touches_the_engine`,
//!    `grid_makes_tiles_and_list_makes_rows_and_the_choice_is_saved_and_restored`
//! 4. Floor: `the_270_floor_and_sparse_rows_hold_while_the_window_narrows`
//! 5. Rail: `the_rail_lists_every_item_under_its_section_and_a_click_brings_a_group_to_the_top`,
//!    `the_last_group_comes_to_the_top_after_the_first_four_are_folded`
//! 6. Dock: `the_six_dock_elements_stay_inside_the_window_at_every_size`,
//!    `the_primary_action_walks_the_output_through_its_states`,
//!    `scrolling_the_timeline_never_moves_the_output_dock`
//! 7. Frames: `both_monitors_paint_the_same_frame_and_resizing_never_recomposes`,
//!    `grid_thumbnails_are_composed_for_groups_far_from_preview`

use super::*;
use std::collections::BTreeSet;

/// What was true once the Engine reached its final live state, plus the
/// running frame bookkeeping, checked again after every step.
struct Recorded {
    state: presentation::state::EngineState,
    revision: u64,
    cue: usize,
    group: usize,
    program: Arc<rendering::Frame>,
    preview: Arc<rendering::Frame>,
    program_id: FrameId,
    ever_ready: BTreeSet<FrameId>,
    composed: usize,
}

fn ready_ids(open: &Open, cx: &VisualTestContext) -> BTreeSet<FrameId> {
    open.view.read_with(cx, |view, _| {
        view.engine
            .snapshot()
            .frame_ids()
            .into_iter()
            .filter(|id| matches!(view.frames.peek(*id), FrameView::Ready { .. }))
            .collect()
    })
}

fn check(
    step: &str,
    open: &Open,
    cx: &mut VisualTestContext,
    (width, height, _): (f32, f32, InterfaceScale),
    rec: &mut Recorded,
    grid: bool,
) {
    let tag = format!("{width}x{height} {step}");
    // (a) the Engine never notices.
    let state = engine_state(open, cx);
    assert_eq!(state, rec.state, "{tag}: EngineState changed");
    assert_eq!(state.revision(), rec.revision, "{tag}: revision moved");

    // (b) the six high-stakes elements are on screen and inside the window.
    for selector in DOCK_ELEMENTS {
        let b = bounds(cx, selector);
        assert!(inside(b, width, height), "{tag}: {selector} at {b:?}");
    }

    // (c) both monitors keep painting the one recorded frame.
    open.view.update(cx, |view, _| {
        let program = view.last_program.frame.clone().expect("Program is Ready");
        let preview = view.last_preview.frame.clone().expect("Preview is Ready");
        assert!(Arc::ptr_eq(&program, &rec.program), "{tag}: Program Arc");
        assert!(Arc::ptr_eq(&preview, &rec.preview), "{tag}: Preview Arc");
        let FrameView::Ready { frame, .. } = view.frames.peek(rec.program_id) else {
            panic!("{tag}: the shown frame left the store");
        };
        assert!(Arc::ptr_eq(&frame, &rec.program), "{tag}: store Arc");
    });

    // (d) an id composed once is never composed again: growth is bounded by
    // the ids that became ready for the first time.
    let ready = ready_ids(open, cx);
    let fresh = ready.difference(&rec.ever_ready).count();
    let composed = open
        .view
        .read_with(cx, |view, _| view.frames.composed_count());
    assert!(
        composed <= rec.composed + fresh,
        "{tag}: composed {} -> {composed} with {fresh} new ids",
        rec.composed
    );
    rec.ever_ready.extend(ready);
    rec.composed = composed;

    // (e) the active tile is on screen in Grid and 16:9.
    if grid {
        assert!(present(cx, cue(rec.cue)), "{tag}: tile cue-{}", rec.cue);
        let frame = bounds(cx, sel(format!("cue-{}-frame", rec.cue)));
        let (w, h) = (f32::from(frame.size.width), f32::from(frame.size.height));
        assert!((h - w * 9.0 / 16.0).abs() < 0.5, "{tag}: tile {w}x{h}");
    }
}

#[gpui::test]
fn live_control_survives_every_size_mode_and_rail_state(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    assert!(run(&open, cx, Command::EnableOutput).is_ok());
    let k = open.view.read_with(cx, |view, _| {
        let snapshot = view.engine.snapshot();
        (snapshot.cue_count() / 2..snapshot.cue_count())
            .find(|flat| {
                matches!(
                    snapshot.cue(*flat).unwrap().behavior(),
                    CueBehavior::Project { .. }
                )
            })
            .expect("a Project Cue in the second half")
    });
    assert!(run(&open, cx, Command::GoTo(k)).is_ok());
    assert!(run(&open, cx, Command::Take).is_ok());
    assert!(run(&open, cx, Command::ToggleFreeze).is_ok());
    settle(&open, cx, 1000);

    let state = engine_state(&open, cx);
    let (program, preview, program_id, group) = open.view.update(cx, |view, _| {
        let id = view.engine.program_view(Millis(1000)).shown.unwrap();
        let snapshot = view.engine.snapshot();
        let group = (0..snapshot.item_count())
            .rev()
            .find(|item| snapshot.item_start(*item).is_some_and(|start| start <= k))
            .unwrap();
        (
            view.last_program.frame.clone().expect("Program is Ready"),
            view.last_preview.frame.clone().expect("Preview is Ready"),
            id,
            group,
        )
    });
    let mut rec = Recorded {
        revision: state.revision(),
        state,
        cue: k,
        group,
        program,
        preview,
        program_id,
        ever_ready: ready_ids(&open, cx),
        composed: open
            .view
            .read_with(cx, |view, _| view.frames.composed_count()),
    };
    let last = open
        .view
        .read_with(cx, |view, _| view.engine.snapshot().item_count() - 1);
    let mut checks = 0;

    for case in SIZES {
        let (width, height, scale) = case;
        open.view
            .update(cx, |view, cx| view.set_environment(environment(scale), cx));
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
        check("resize", &open, cx, case, &mut rec, true);

        for (name, button) in [("List", "button-List"), ("Grid", "button-Grid")] {
            click(cx, button);
            check(name, &open, cx, case, &mut rec, name == "Grid");
            checks += 1;
        }

        if present(cx, "button-Unpin Ordo") {
            // Wide: unpin, then pin again.
            click(cx, "button-Unpin Ordo");
            check("unpin", &open, cx, case, &mut rec, true);
            open_overlay(&open, cx);
            click(cx, "button-Pin Ordo");
            check("pin", &open, cx, case, &mut rec, true);
        } else {
            // Compact or Constrained: open the overlay, close it by the scrim.
            open_overlay(&open, cx);
            check("overlay open", &open, cx, case, &mut rec, true);
            let panel = bounds(cx, "presenter-rail");
            let at = gpui::point(px(right(panel) + 20.0), px(height - 8.0));
            cx.simulate_click(at, Modifiers::default());
            cx.run_until_parked();
            assert!(!present(cx, "presenter-rail"), "the scrim closes it");
            check("overlay closed", &open, cx, case, &mut rec, true);
        }
        checks += 2;

        open.view
            .update(cx, |view, cx| view.jump_to_group(last, cx));
        cx.run_until_parked();
        animate(&open, cx, 0);
        check("jump last", &open, cx, case, &mut rec, false);
        // Bring the active Cue's group back so its tile is on screen.
        let back = rec.group;
        open.view
            .update(cx, |view, cx| view.jump_to_group(back, cx));
        cx.run_until_parked();
        animate(&open, cx, 0);
        check("jump back", &open, cx, case, &mut rec, true);
        checks += 3;
        println!(
            "e2e {width}x{height}@{scale:?}: ok, {} frames composed",
            rec.composed
        );
    }
    println!(
        "e2e: {checks} checkpoints, Cue {k} in group {}, revision {}",
        rec.group, rec.revision
    );
}
