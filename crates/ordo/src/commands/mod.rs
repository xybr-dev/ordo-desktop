//! The one seam through which a Presenter control reaches the Engine.
//!
//! Every button, and later every key, runs a [`Command`] through
//! [`dispatch`]; nothing else in the Presenter view calls `Engine::apply`.
//! T1.4 bound the buttons. T1.5 binds the product shortcuts through the
//! [`registry`] to this same function, so the refusal and "unchanged"
//! rules stay in one place.

pub mod registry;

use presentation::{
    commands::Command,
    engine::{Engine, Outcome, Refusal},
    state::Millis,
};

/// Apply `command` at `now`. A refusal changes nothing: the Engine state and
/// revision are exactly as before, and the caller says why in plain words.
pub fn dispatch(engine: &mut Engine, command: Command, now: Millis) -> Result<Outcome, Refusal> {
    engine.apply(command, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presenter::frames::test_support::fixture_snapshot;
    use std::sync::Arc;

    fn engine() -> Engine {
        Engine::new(Arc::new(fixture_snapshot().snapshot().clone()))
    }

    #[test]
    fn dispatch_applies_a_command_exactly_as_the_engine_does() {
        let mut through_seam = engine();
        let mut direct = engine();
        for command in [Command::EnableOutput, Command::Next, Command::Take] {
            assert_eq!(
                dispatch(&mut through_seam, command, Millis(10)),
                direct.apply(command, Millis(10))
            );
        }
        assert_eq!(through_seam.state(), direct.state());
    }

    #[test]
    fn a_refused_dispatch_leaves_state_and_revision_alone() {
        let mut engine = engine();
        let before = engine.state().clone();
        assert_eq!(
            dispatch(&mut engine, Command::Take, Millis(0)),
            Err(Refusal::OutputDisabled)
        );
        assert_eq!(engine.state(), &before);
        assert_eq!(engine.state().revision(), before.revision());
    }
}
