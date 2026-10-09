//! Presenter command sequences, driven through the public Module Interface.

use presentation::presenter::{PresenterCommand, PresenterEngine, PresenterState, coalesce};

use PresenterCommand::*;

// Deliberately duplicated from ordo's bench (crates/ordo/src/performance/bench.rs)
// to keep this test crate free of an `ordo` dependency edge.
/// Deterministic xorshift64: no dependencies, identical on every run.
struct XorShift(u64);

impl XorShift {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

fn random_command(rng: &mut XorShift, cue_count: usize) -> PresenterCommand {
    match rng.below(10) {
        0..=2 => Next,
        3..=4 => Previous,
        5 => GoTo(rng.below(cue_count as u64 + 3) as usize),
        6 => TakeLive,
        7 => ToggleFreeze,
        8 => ToggleBlackout,
        _ => GoTo(rng.below(cue_count as u64) as usize),
    }
}

fn random_sequence(rng: &mut XorShift, cue_count: usize, len: usize) -> Vec<PresenterCommand> {
    (0..len).map(|_| random_command(rng, cue_count)).collect()
}

fn run(cue_count: usize, commands: &[PresenterCommand]) -> PresenterState {
    let mut engine = PresenterEngine::new(cue_count);
    let mut last = *engine.state();
    for &command in commands {
        last = *engine.apply(command);
    }
    last
}

/// Every field a congregation or operator can observe. Revision is excluded
/// on purpose: it counts state changes, and coalescing performs fewer.
fn observable(state: &PresenterState) -> (usize, Option<usize>, bool, bool, Option<usize>) {
    (
        state.preview(),
        state.program(),
        state.frozen(),
        state.blackout(),
        state.program_visible(),
    )
}

#[test]
fn rapid_navigation_is_applied_in_order() {
    let mut engine = PresenterEngine::new(5);
    let previews: Vec<usize> = [Next, Next, Next, Previous, GoTo(4), Next, Previous]
        .into_iter()
        .map(|command| engine.apply(command).preview())
        .collect();
    assert_eq!(previews, vec![1, 2, 3, 2, 4, 4, 3]);
}

#[test]
fn take_freeze_and_blackout_interleave_in_order() {
    let mut engine = PresenterEngine::new(4);
    engine.apply(GoTo(1));
    engine.apply(TakeLive);
    engine.apply(ToggleFreeze);
    engine.apply(Next);
    engine.apply(ToggleBlackout);
    assert_eq!(engine.state().program(), Some(1));
    assert_eq!(engine.state().preview(), 2);
    assert_eq!(engine.state().program_visible(), None);

    let state = *engine.apply(TakeLive);
    assert_eq!(observable(&state), (2, Some(2), false, false, Some(2)));
}

#[test]
fn coalesced_and_raw_sequences_reach_identical_final_state() {
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for case in 0..2_000 {
        let cue_count = 1 + rng.below(12) as usize;
        let len = rng.below(120) as usize;
        let commands = random_sequence(&mut rng, cue_count, len);
        let collapsed = coalesce(&commands, cue_count, 0);

        let raw = run(cue_count, &commands);
        let fast = run(cue_count, &collapsed);
        assert_eq!(
            observable(&raw),
            observable(&fast),
            "case {case}: cue_count {cue_count}, raw {commands:?}, coalesced {collapsed:?}"
        );
        assert!(
            raw.revision() >= fast.revision(),
            "case {case}: collapsing never adds state changes"
        );
    }
}

#[test]
fn coalescing_never_loses_or_reorders_barriers() {
    let mut rng = XorShift(0xD1B5_4A32_D192_ED03);
    for _ in 0..2_000 {
        let cue_count = 1 + rng.below(12) as usize;
        let len = rng.below(120) as usize;
        let commands = random_sequence(&mut rng, cue_count, len);
        let collapsed = coalesce(&commands, cue_count, 0);

        let barriers = |sequence: &[PresenterCommand]| -> Vec<PresenterCommand> {
            sequence
                .iter()
                .copied()
                .filter(|command| !command.is_navigation())
                .collect()
        };
        assert_eq!(barriers(&commands), barriers(&collapsed));
        assert!(collapsed.len() <= commands.len());
    }
}

#[test]
fn final_intent_survives_a_trailing_navigation_run() {
    let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
    for _ in 0..1_000 {
        let cue_count = 1 + rng.below(12) as usize;
        let mut commands = random_sequence(&mut rng, cue_count, 40);
        commands.push(TakeLive);
        // Only navigation after the take, so the trailing run is what the
        // coalesced sequence must end with.
        commands.extend(
            random_sequence(&mut rng, cue_count, 30)
                .into_iter()
                .filter(|command| command.is_navigation()),
        );
        commands.push(Next);

        let raw = run(cue_count, &commands);
        let collapsed = coalesce(&commands, cue_count, 0);
        let fast = run(cue_count, &collapsed);

        assert_eq!(observable(&raw), observable(&fast));
        // The operator's last Preview choice is exactly what the trailing
        // GoTo carries, so nothing after the take was dropped.
        assert_eq!(collapsed.last(), Some(&GoTo(raw.preview())));
    }
}
