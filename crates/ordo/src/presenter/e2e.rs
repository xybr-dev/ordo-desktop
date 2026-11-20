//! Presenter live-control E2E (T1.4 WP8) and keyboard E2E (T1.5 WP6, at the end of
//! this file: every bound key in every focus place, with `registry::allows` as
//! the oracle and a sent-Command log as the evidence).
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

// ---------------------------------------------------------------------------
// Keyboard E2E (T1.5 WP6)
// ---------------------------------------------------------------------------

use crate::commands::registry::{self, CommandId, FocusContext, Scope};
use presentation::state::ProgramContent;

/// One place keyboard focus can be in the Presenter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Where {
    Timeline,
    Dock,
    /// The Dock after Freeze was pressed with the mouse (state is Frozen).
    DockAfterFreeze,
    PinnedRail,
    RailOverlay,
    ShortcutReference,
    AutoPlayMenu,
    HeaderControl,
    TextField,
    /// A real `ui` dialog (gpui-base Sheet) opened inside the Presenter.
    Modal,
}

impl Where {
    /// The focus context the registry policy must see there.
    fn context(self) -> FocusContext {
        match self {
            Where::Timeline => FocusContext::Timeline,
            Where::Dock | Where::DockAfterFreeze | Where::AutoPlayMenu => FocusContext::Dock,
            Where::PinnedRail | Where::RailOverlay => FocusContext::Rail,
            Where::ShortcutReference => FocusContext::ShortcutReference,
            Where::HeaderControl => FocusContext::Elsewhere,
            Where::TextField => FocusContext::TextEntry,
            Where::Modal => FocusContext::Modal,
        }
    }
}

/// Put the Engine and the surfaces back to one known state: nothing open, no
/// auto-play, not Frozen or in Blackout, the clock past every fade, Preview on
/// a Cue that is not on Program (so Take has something to do), no status.
fn reset(open: &Open, cx: &mut VisualTestContext, now: &mut u64) {
    // The menu may be left open by the last key (with focus elsewhere, so Esc
    // would not reach it), or still be closing: a press outside dismisses it.
    if menu_open(cx) {
        let outside = bounds(cx, "button-List").center();
        cx.simulate_click(outside, Modifiers::default());
        animate(open, cx, 400);
        assert!(!menu_open(cx), "a press outside closes the open menu");
    }
    close_modals(open, cx);
    animate(open, cx, 400);
    open.view.update_in(
        cx,
        |view, window, cx| {
            while view.close_topmost(window, cx) {}
        },
    );
    cx.run_until_parked();
    *now += 2000;
    settle(open, cx, *now);
    let _ = run(open, cx, Command::StopAutoPlay);
    match live(open, cx) {
        LiveState::Frozen => drop(run(open, cx, Command::ToggleFreeze)),
        LiveState::Blackout => drop(run(open, cx, Command::ToggleBlackout)),
        _ => {}
    }
    *now += 2000;
    settle(open, cx, *now);
    assert!(
        !matches!(live(open, cx), LiveState::Frozen | LiveState::Blackout),
        "reset releases Freeze and Blackout"
    );
    let on_program = open
        .view
        .read_with(cx, |view, _| match view.engine.state().program() {
            ProgramContent::Frame { cue, .. } => Some(cue),
            ProgramContent::Black => None,
        });
    let target = if on_program == Some(1) { 2 } else { 1 };
    run(open, cx, Command::GoTo(target)).unwrap();
    assert_eq!(status(open, cx), None, "reset leaves no refusal text");
}

/// Reset, then move focus to `place`, and check the policy reads it right.
fn arrange(open: &Open, cx: &mut VisualTestContext, place: Where, now: &mut u64) {
    reset(open, cx, now);
    match place {
        Where::Timeline => focus_timeline_now(open, cx),
        Where::Dock => open
            .view
            .update_in(cx, |view, window, cx| view.dock_focus.focus(window, cx)),
        Where::DockAfterFreeze => {
            click(cx, "button-Freeze");
            assert_eq!(live(open, cx), LiveState::Frozen);
        }
        Where::PinnedRail => {
            assert!(present(cx, "presenter-rail"), "the rail is pinned");
            open.view
                .update_in(cx, |view, window, cx| view.rail_focus.focus(window, cx));
        }
        Where::RailOverlay => {
            open_overlay(open, cx);
            open.view
                .update_in(cx, |view, window, cx| view.rail_focus.focus(window, cx));
        }
        Where::ShortcutReference => {
            open.view
                .update_in(cx, |view, window, cx| view.open_reference(window, cx));
            cx.run_until_parked();
        }
        Where::AutoPlayMenu => {
            click(cx, "button-Auto-play off");
            assert!(menu_open(cx), "the Auto-play menu is open");
        }
        // Pressed, not released: a full click hands focus back to the Cues.
        Where::HeaderControl => press_without_release(cx, "button-List"),
        Where::Modal => open_modal(open, cx),
        Where::TextField => {
            let field = focus_text_field(open, cx);
            cx.update(|window, cx| field.set_value("", window, cx));
        }
    }
    cx.run_until_parked();
    assert_eq!(
        focus_context_now(open, cx),
        place.context(),
        "{place:?}: focus context"
    );
}

/// Every key the registry binds to a GPUI action here, as
/// `(entry, keystroke)`, in table order.
fn bound_keys() -> Vec<(&'static registry::Entry, &'static str)> {
    registry::ENTRIES
        .iter()
        .filter(|entry| match entry.scope {
            Scope::Surface => entry.id == CommandId::ToggleReference,
            Scope::Editing => false,
            Scope::Output | Scope::Operating => true,
        })
        .flat_map(|entry| entry.keys.iter().map(move |keys| (entry, *keys)))
        .collect()
}

/// Presses every bound key in every place and checks, with `registry::allows`
/// as the only oracle, which keys reached the Engine (or its refusal text).
fn sweep(cx: &mut TestAppContext, case: (f32, f32, InterfaceScale), places: &[Where]) {
    let (open, cx) = open(cx, case);
    bind_registry_keys(cx);
    assert!(run(&open, cx, Command::EnableOutput).is_ok());
    let mut now = 1000;
    reset(&open, cx, &mut now);
    assert!(run(&open, cx, Command::Take).is_ok());
    let mut presses = 0;

    for &place in places {
        for (entry, keys) in bound_keys() {
            arrange(&open, cx, place, &mut now);
            let context = focus_context_now(&open, cx);
            let before = engine_state(&open, cx);
            open.view
                .read_with(cx, |view, _| view.sent.borrow_mut().clear());
            let was_open = reference_open(&open, cx);
            let tag = format!("{place:?} / {keys}");

            press_keys(cx, keys);
            presses += 1;

            let after = engine_state(&open, cx);
            let sent = open
                .view
                .read_with(cx, |view, _| view.sent.borrow().clone());
            let reached = !sent.is_empty();
            match entry.id {
                // Ctrl+/ is chorded: it toggles the reference everywhere
                // except a true modal, and never touches the Engine.
                CommandId::ToggleReference => {
                    assert_eq!(after, before, "{tag}: Ctrl+/ never reaches the Engine");
                    let toggled = reference_open(&open, cx) != was_open;
                    assert_eq!(
                        toggled,
                        place != Where::Modal,
                        "{tag}: toggles everywhere except under a true modal"
                    );
                }
                // `O` is view-only: it is allowed or not, and shows as focus
                // arriving in the rail, never as an Engine change.
                CommandId::OpenOrdo => {
                    assert_eq!(after, before, "{tag}: O never reaches the Engine");
                    let in_rail = focus_context_now(&open, cx) == FocusContext::Rail;
                    let arrived = in_rail && context != FocusContext::Rail;
                    if context == FocusContext::Rail {
                        assert!(in_rail, "{tag}: focus stays in the rail");
                    }
                    assert_eq!(
                        arrived,
                        registry::allows(entry.scope, context),
                        "{tag}: O reaches the rail only where allowed"
                    );
                }
                _ => {
                    let allowed = registry::allows(entry.scope, context);
                    assert_eq!(
                        reached, allowed,
                        "{tag}: reached the Engine = {reached}, policy says {:?} for {context:?}",
                        entry.scope
                    );
                    if allowed {
                        // Exactly the one command the registry maps the key to.
                        let want = registry::command_for(entry.id).unwrap();
                        assert_eq!(sent, [want], "{tag}");
                    } else {
                        assert_eq!(after, before, "{tag}: the Engine changed");
                        assert_eq!(status(&open, cx), None, "{tag}: a refusal was shown");
                    }
                }
            }
        }

        // The keys the surface and region systems own never reach the Engine
        // from anywhere (their own behaviour has its own tests).
        for keys in ["escape", "f6", "shift-f6", "tab", "shift-tab"] {
            arrange(&open, cx, place, &mut now);
            let before = engine_state(&open, cx);
            press_keys(cx, keys);
            assert_eq!(
                engine_state(&open, cx),
                before,
                "{place:?} / {keys}: reached the Engine"
            );
            assert_eq!(status(&open, cx), None, "{place:?} / {keys}");
            presses += 1;
        }
    }
    println!("keyboard e2e {case:?}: {presses} key presses over {places:?}");
}

#[gpui::test]
fn wide_every_registered_key_reaches_the_engine_only_where_the_focus_context_allows(
    cx: &mut TestAppContext,
) {
    sweep(
        cx,
        SIZES[0],
        &[
            Where::Timeline,
            Where::Dock,
            Where::DockAfterFreeze,
            Where::PinnedRail,
            Where::ShortcutReference,
            Where::AutoPlayMenu,
            Where::HeaderControl,
            Where::TextField,
            Where::Modal,
        ],
    );
}

#[gpui::test]
fn constrained_every_registered_key_reaches_the_engine_only_where_the_focus_context_allows(
    cx: &mut TestAppContext,
) {
    sweep(
        cx,
        SIZES[2],
        &[
            Where::Timeline,
            Where::Dock,
            Where::RailOverlay,
            Where::ShortcutReference,
            Where::AutoPlayMenu,
            Where::HeaderControl,
            Where::TextField,
            Where::Modal,
        ],
    );
}

/// Typing `fbo` and spaces into a real field puts the letters in the field and
/// sends the Engine nothing, while leaving the field gives the keys back.
#[gpui::test]
fn typing_f_b_o_and_spaces_into_a_text_field_edits_the_field_only(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[1]);
    bind_registry_keys(cx);
    assert!(run(&open, cx, Command::EnableOutput).is_ok());
    let mut now = 1000;
    arrange(&open, cx, Where::TextField, &mut now);
    let field = focus_text_field(&open, cx);
    let before = engine_state(&open, cx);
    open.view
        .read_with(cx, |view, _| view.sent.borrow_mut().clear());
    press_keys(cx, "f b o space f b o space space");
    assert!(
        open.view
            .read_with(cx, |view, _| view.sent.borrow().is_empty())
    );
    assert_eq!(
        cx.update(|_, cx| field.value(cx)).as_ref(),
        "fbo fbo  ",
        "the letters and spaces landed in the field"
    );
    assert_eq!(engine_state(&open, cx), before);
    assert_eq!(status(&open, cx), None);
    assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
    focus_timeline_now(&open, cx);
    press_keys(cx, "f");
    assert_eq!(
        open.view
            .read_with(cx, |view, _| view.sent.borrow().clone()),
        [Command::ToggleFreeze],
        "the Cues get F back"
    );
}

/// The Recovery Drill replayed through `PresenterView::command` and
/// `PresenterView::tick` on the injected clock: after every line the Engine
/// state formats exactly as the committed `commands.expect` line. This is the
/// behaviour half of `live-control-lab --run-drill`; it measures no time.
#[gpui::test]
fn the_recovery_drill_replays_through_the_presenter_with_no_mismatch(cx: &mut TestAppContext) {
    use presentation::snapshot_reader::{DirectorySource, read_snapshot};
    use presentation::test_support::drill::{
        COMMAND_MS, Step, parse_step, script_lines, state_line,
    };
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/presentations/recovery-drill");
    let snapshot =
        Arc::new(read_snapshot(&DirectorySource::new(dir.clone())).expect("drill loads"));
    let script = std::fs::read_to_string(dir.join("commands.txt")).unwrap();
    let expected = std::fs::read_to_string(dir.join("commands.expect")).unwrap();
    let (open, cx) = open_snapshot(cx, SIZES[0], UiPreferences::default(), snapshot);
    let mut clock = 0;
    let mut mismatches = Vec::new();
    for (index, (text, want)) in script_lines(&script).zip(expected.lines()).enumerate() {
        let outcome = match parse_step(text).expect("a known drill line") {
            Step::Command(command) => {
                clock += COMMAND_MS;
                open.now.set(clock);
                run(&open, cx, command)
            }
            Step::Tick(ms) => {
                clock += ms;
                let before = engine_state(&open, cx);
                settle(&open, cx, clock);
                Ok(if before == engine_state(&open, cx) {
                    Outcome::Unchanged
                } else {
                    Outcome::Changed
                })
            }
        };
        let got = open.view.read_with(cx, |view, _| {
            state_line(index + 1, text, view.engine.state(), outcome)
        });
        if got != want {
            mismatches.push(format!("expected `{want}`, got `{got}`"));
        }
    }
    assert_eq!(script_lines(&script).count(), expected.lines().count());
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}
