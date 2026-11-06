//! The one command registry: every Presenter key, its keycap text, its label
//! and the focus contexts it may run in come from [`ENTRIES`].
//!
//! GPUI key bindings ([`key_bindings`]) and, later, the Keyboard shortcuts
//! reference are both derived from this table, so a key can never be bound
//! without being listed or listed without being bound. Entries whose [`Scope`]
//! is [`Scope::Surface`] are listed for the reference but bound elsewhere
//! (`Esc` by the surface stack, `F6` by the region registry, `Ctrl+/` by the
//! reference) and never reach the Engine.
//!
//! A key reaches the Engine only through [`command_for`] and then
//! [`super::dispatch`]. Two guards keep single keys out of text entry and
//! modals: the binding predicate (`Presenter && !TextEntry && !Modal`, evaluated
//! by GPUI against every context on the focus stack) and [`allows`], which the
//! view checks again before sending anything.

use gpui::{Action, KeyBinding, actions};
use presentation::commands::Command;
use std::rc::Rc;

actions!(
    presenter,
    [
        Previous,
        Next,
        PreviousItem,
        NextItem,
        First,
        Last,
        TakeLive,
        ToggleFreeze,
        ToggleBlackout,
        ToggleAutoPlay,
        OpenOrdo,
    ]
);

/// The GPUI key context the Presenter root declares.
pub const PRESENTER_CONTEXT: &str = "Presenter";

/// Active only inside the Presenter and never while a text field or a modal
/// dialog anywhere above focus declares its own context (`!X` is false if any
/// context on the focus stack matches `X`).
pub const BINDING_PREDICATE: &str = "Presenter && !TextEntry && !Modal";

/// Which table row a key belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    Previous,
    Next,
    PreviousItem,
    NextItem,
    First,
    Last,
    TakeLive,
    ToggleFreeze,
    ToggleBlackout,
    ToggleAutoPlay,
    OpenOrdo,
    /// `Esc`: closes the topmost temporary surface.
    CloseSurface,
    /// `F6` / `Shift+F6`: cycles the visible Presenter regions.
    CycleRegion,
    /// `Ctrl+/`: shows or hides the shortcut reference.
    ToggleReference,
}

/// The reference group an entry is listed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Presenter,
}

/// Where a key may run (see [`allows`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// `F`, `B`, `Space`: also active over passive surfaces and in the
    /// Presenter's other controls (the header).
    Output,
    /// Navigation, `A`, `O`: only while the Cues or the Output controls have
    /// focus.
    Operating,
    /// `Esc`, `F6`, `Ctrl+/`: owned by the surface or region system; listed in
    /// the reference, never an Engine command.
    Surface,
}

/// Where keyboard focus is, as far as key policy cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusContext {
    Timeline,
    Dock,
    Rail,
    ShortcutReference,
    /// A text field or emergency editor.
    TextEntry,
    /// A true modal dialog.
    Modal,
    /// Anywhere else inside the Presenter (the header, nothing focused):
    /// Output keys only.
    Elsewhere,
}

/// One registered key.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub id: CommandId,
    /// GPUI keystrokes, one binding each.
    pub keys: &'static [&'static str],
    /// The keycap text shown to the operator.
    pub keycaps: &'static str,
    pub label: &'static str,
    pub group: Group,
    pub scope: Scope,
}

const fn entry(
    id: CommandId,
    keys: &'static [&'static str],
    keycaps: &'static str,
    label: &'static str,
    scope: Scope,
) -> Entry {
    Entry {
        id,
        keys,
        keycaps,
        label,
        group: Group::Presenter,
        scope,
    }
}

/// Every Presenter key (`MVP-PLAN.md` section 8.2). `Insert`, `Ctrl+E` and
/// `Ctrl+R` belong to emergency editing and are not registered yet.
pub static ENTRIES: &[Entry] = &[
    entry(
        CommandId::Previous,
        &["left", "up"],
        "Left / Up",
        "Previous Cue",
        Scope::Operating,
    ),
    entry(
        CommandId::Next,
        &["right", "down"],
        "Right / Down",
        "Next Cue",
        Scope::Operating,
    ),
    entry(
        CommandId::PreviousItem,
        &["pageup"],
        "Page Up",
        "Previous Mass item",
        Scope::Operating,
    ),
    entry(
        CommandId::NextItem,
        &["pagedown"],
        "Page Down",
        "Next Mass item",
        Scope::Operating,
    ),
    entry(
        CommandId::First,
        &["home"],
        "Home",
        "First Cue",
        Scope::Operating,
    ),
    entry(
        CommandId::Last,
        &["end"],
        "End",
        "Last Cue",
        Scope::Operating,
    ),
    entry(
        CommandId::ToggleFreeze,
        &["f"],
        "F",
        "Freeze or release Freeze",
        Scope::Output,
    ),
    entry(
        CommandId::ToggleBlackout,
        &["b"],
        "B",
        "Blackout or release Blackout",
        Scope::Output,
    ),
    entry(
        CommandId::ToggleAutoPlay,
        &["a"],
        "A",
        "Start or stop auto-play",
        Scope::Operating,
    ),
    entry(
        CommandId::OpenOrdo,
        &["o"],
        "O",
        "Open the Ordo rail",
        Scope::Operating,
    ),
    entry(
        CommandId::TakeLive,
        &["space"],
        "Space",
        "Take Preview live",
        Scope::Output,
    ),
    entry(
        CommandId::CloseSurface,
        &["escape"],
        "Esc",
        "Close the open panel",
        Scope::Surface,
    ),
    entry(
        CommandId::CycleRegion,
        &["f6", "shift-f6"],
        "F6 / Shift+F6",
        "Next or previous region",
        Scope::Surface,
    ),
    entry(
        CommandId::ToggleReference,
        &["ctrl-/"],
        "Ctrl+/",
        "Keyboard shortcuts",
        Scope::Surface,
    ),
];

/// The table row for `id`.
pub fn entry_for(id: CommandId) -> &'static Entry {
    ENTRIES
        .iter()
        .find(|entry| entry.id == id)
        .expect("every CommandId has a row in ENTRIES")
}

/// Whether a key of `scope` may act while focus is in `context`.
pub fn allows(scope: Scope, context: FocusContext) -> bool {
    use FocusContext::*;
    match scope {
        Scope::Output => matches!(
            context,
            Timeline | Dock | Rail | ShortcutReference | Elsewhere
        ),
        Scope::Operating => matches!(context, Timeline | Dock),
        Scope::Surface => false,
    }
}

/// The Engine command a key sends, or `None` for a key that is view-only
/// (`O`) or owned by a surface.
pub fn command_for(id: CommandId) -> Option<Command> {
    match id {
        CommandId::Previous => Some(Command::Previous),
        CommandId::Next => Some(Command::Next),
        CommandId::PreviousItem => Some(Command::PreviousItem),
        CommandId::NextItem => Some(Command::NextItem),
        CommandId::First => Some(Command::First),
        CommandId::Last => Some(Command::Last),
        CommandId::TakeLive => Some(Command::Take),
        CommandId::ToggleFreeze => Some(Command::ToggleFreeze),
        CommandId::ToggleBlackout => Some(Command::ToggleBlackout),
        CommandId::ToggleAutoPlay => Some(Command::ToggleAutoPlay),
        CommandId::OpenOrdo
        | CommandId::CloseSurface
        | CommandId::CycleRegion
        | CommandId::ToggleReference => None,
    }
}

/// Calls `$callback!` with the name of every command that has a GPUI action.
/// Each name is both a [`CommandId`] variant and an action type of the same
/// name, so the action mapping here and the view's listeners are generated
/// from this one list.
macro_rules! for_each_bound_command {
    ($callback:ident) => {
        $callback! {
            Previous,
            Next,
            PreviousItem,
            NextItem,
            First,
            Last,
            TakeLive,
            ToggleFreeze,
            ToggleBlackout,
            ToggleAutoPlay,
            OpenOrdo
        }
    };
}
pub(crate) use for_each_bound_command;

/// The GPUI action an Output or Operating entry binds, `None` for a Surface
/// entry (bound elsewhere).
fn action_for(id: CommandId) -> Option<Box<dyn Action>> {
    macro_rules! arms {
        ($($name:ident),*) => {
            match id {
                $(CommandId::$name => Some(Box::new($name) as Box<dyn Action>),)*
                CommandId::CloseSurface | CommandId::CycleRegion | CommandId::ToggleReference => {
                    None
                }
            }
        };
    }
    for_each_bound_command!(arms)
}

/// The GPUI bindings for every Output and Operating entry, all under
/// [`BINDING_PREDICATE`].
pub fn key_bindings() -> Vec<KeyBinding> {
    let predicate: Rc<gpui::KeyBindingContextPredicate> = Rc::new(
        gpui::KeyBindingContextPredicate::parse(BINDING_PREDICATE)
            .expect("the binding predicate parses"),
    );
    let mut bindings = Vec::new();
    for entry in ENTRIES {
        let Some(action) = action_for(entry.id) else {
            continue;
        };
        for keys in entry.keys {
            bindings.push(
                KeyBinding::load(
                    keys,
                    action.boxed_clone(),
                    Some(predicate.clone()),
                    false,
                    None,
                    &gpui::DummyKeyboardMapper,
                )
                .expect("every registered keystroke parses"),
            );
        }
    }
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const ALL_CONTEXTS: [FocusContext; 7] = [
        FocusContext::Timeline,
        FocusContext::Dock,
        FocusContext::Rail,
        FocusContext::ShortcutReference,
        FocusContext::TextEntry,
        FocusContext::Modal,
        FocusContext::Elsewhere,
    ];

    #[test]
    fn every_keystroke_parses() {
        for entry in ENTRIES {
            assert!(!entry.keys.is_empty(), "{:?} has no key", entry.id);
            for keys in entry.keys {
                assert!(
                    gpui::Keystroke::parse(keys).is_ok(),
                    "{keys:?} of {:?} does not parse",
                    entry.id
                );
            }
        }
        assert!(
            gpui::KeyBindingContextPredicate::parse(BINDING_PREDICATE).is_ok(),
            "the binding predicate parses"
        );
    }

    #[test]
    fn no_keystroke_is_registered_twice() {
        let mut seen = BTreeSet::new();
        for entry in ENTRIES {
            for keys in entry.keys {
                assert!(seen.insert(*keys), "{keys:?} is registered twice");
            }
        }
        let ids: BTreeSet<String> = ENTRIES.iter().map(|e| format!("{:?}", e.id)).collect();
        assert_eq!(ids.len(), ENTRIES.len(), "one row per command");
    }

    #[test]
    fn bindings_cover_every_output_and_operating_entry() {
        let bindings = key_bindings();
        let bound: BTreeSet<String> = bindings
            .iter()
            .flat_map(|b| b.keystrokes().iter().map(|k| k.inner().unparse()))
            .collect();
        let mut expected = BTreeSet::new();
        for entry in ENTRIES {
            for keys in entry.keys {
                if entry.scope == Scope::Surface {
                    assert!(!bound.contains(*keys), "{keys:?} is bound elsewhere");
                } else {
                    expected.insert(gpui::Keystroke::parse(keys).unwrap().unparse());
                }
            }
        }
        assert_eq!(bound, expected);
        assert_eq!(
            bindings.len(),
            ENTRIES
                .iter()
                .filter(|e| e.scope != Scope::Surface)
                .map(|e| e.keys.len())
                .sum::<usize>()
        );
    }

    #[test]
    fn every_binding_carries_the_text_entry_and_modal_guard() {
        for binding in key_bindings() {
            let predicate = binding.predicate().expect("a context predicate");
            assert_eq!(predicate.to_string(), "Presenter && !TextEntry && !Modal");
        }
    }

    /// `ctrl-shift-1` never fired on Linux (see `ui`): no registered key may
    /// ask for shift with a key shift already rewrites.
    #[test]
    fn no_binding_asks_for_shift_with_a_key_shift_already_rewrites() {
        for binding in key_bindings() {
            for keystroke in binding.keystrokes() {
                let keystroke = keystroke.inner();
                let unreachable = keystroke.modifiers.shift
                    && keystroke.key.chars().count() == 1
                    && keystroke.key.to_lowercase() == keystroke.key.to_uppercase();
                assert!(!unreachable, "{keystroke:?} can never be typed");
            }
        }
        for entry in ENTRIES {
            for keys in entry.keys {
                let keystroke = gpui::Keystroke::parse(keys).unwrap();
                let unreachable = keystroke.modifiers.shift
                    && keystroke.key.chars().count() == 1
                    && keystroke.key.to_lowercase() == keystroke.key.to_uppercase();
                assert!(!unreachable, "{keys:?} can never be typed");
            }
        }
    }

    #[test]
    fn allows_follows_the_policy_table() {
        use FocusContext::*;
        for context in ALL_CONTEXTS {
            let output = matches!(
                context,
                Timeline | Dock | Rail | ShortcutReference | Elsewhere
            );
            let operating = matches!(context, Timeline | Dock);
            assert_eq!(allows(Scope::Output, context), output, "{context:?}");
            assert_eq!(allows(Scope::Operating, context), operating, "{context:?}");
            assert!(!allows(Scope::Surface, context), "{context:?}");
        }
        for context in [TextEntry, Modal] {
            for scope in [Scope::Output, Scope::Operating, Scope::Surface] {
                assert!(!allows(scope, context), "{scope:?} in {context:?}");
            }
        }
    }

    #[test]
    fn surface_and_view_only_entries_send_no_engine_command() {
        for entry in ENTRIES {
            let command = command_for(entry.id);
            match entry.scope {
                Scope::Surface => assert!(command.is_none(), "{:?}", entry.id),
                _ if entry.id == CommandId::OpenOrdo => assert!(command.is_none()),
                _ => assert!(command.is_some(), "{:?}", entry.id),
            }
        }
    }

    #[test]
    fn every_command_id_has_one_row() {
        assert_eq!(entry_for(CommandId::TakeLive).keycaps, "Space");
        assert_eq!(entry_for(CommandId::ToggleReference).keys, &["ctrl-/"]);
    }
}
