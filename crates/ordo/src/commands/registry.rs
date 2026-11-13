//! The one command registry: every Presenter key, its keycap text, its label
//! and the focus contexts it may run in come from [`ENTRIES`].
//!
//! GPUI key bindings ([`key_bindings`]) and the Keyboard shortcuts reference
//! (`crate::shortcut_reference`) are both derived from this table, so a key can
//! never be bound without being listed or listed without being bound. Entries
//! whose [`Scope`] is [`Scope::Surface`] are listed for the reference and never
//! reach the Engine. `Ctrl+/` is bound here (it toggles the reference); `Esc`
//! is handled by the surface stack, and `F6`, `Tab` and the Interface-scale
//! keys are bound by `ui::key_bindings`, which a test here checks against this
//! table so the reference cannot drift from them.
//!
//! A key reaches the Engine only through [`command_for`] and then
//! [`super::dispatch`]. Two guards keep single keys out of text entry and
//! modals: the binding predicate ([`binding_predicate`], evaluated by GPUI
//! against every context on the focus stack) and [`allows`], which the
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
        ToggleReference,
    ]
);

actions!(
    editing,
    [
        /// Editor-level Undo in an edit session (`Ctrl+Z`).
        Undo,
        /// Editor-level Redo in an edit session (`Ctrl+Y`).
        Redo,
        /// Moves the selected section one place up (`Alt+Up`).
        MoveUp,
        /// Moves the selected section one place down (`Alt+Down`).
        MoveDown,
    ]
);

/// The GPUI key context an edit session's container declares; the Editing
/// keys bind here and never in [`PRESENTER_CONTEXT`].
pub const EDIT_SESSION_CONTEXT: &str = "EditSession";

/// The key context the sections list declares; `Alt+Up` and `Alt+Down` bind
/// here so they move a section only while the list has focus.
pub const LYRICS_SECTIONS_CONTEXT: &str = "LyricsSections";

/// The key context an Editing entry binds under.
pub fn editing_context(id: CommandId) -> &'static str {
    match id {
        CommandId::MoveUp | CommandId::MoveDown => LYRICS_SECTIONS_CONTEXT,
        _ => EDIT_SESSION_CONTEXT,
    }
}

/// The GPUI key context the Presenter root declares.
pub const PRESENTER_CONTEXT: &str = "Presenter";

/// Key contexts that mean "the operator is typing". `TextEntry` is Ordo's own
/// (emergency editor, tests); the rest are what `gpui-base` declares on its
/// editable controls: `Input` (every text input, which is what a `ui` Field is
/// built on), `NumberInput`, `Combobox` (type-ahead) and `TimeField` (typed
/// digits). `TextView` is read-only text and `Select`, `Popover`, `Tree` and
/// `Root` are not text entry, so they are not listed (`Root` wraps the whole
/// window and would suspend every key).
pub const TEXT_ENTRY_CONTEXTS: &[&str] =
    &["TextEntry", "Input", "NumberInput", "Combobox", "TimeField"];

/// Key contexts of true modals, which own every key until closed. `Modal` is
/// Ordo's own; `Dialog` and `Sheet` are `gpui-base`'s (`DESIGN.md`: "A focused
/// dialog or slide-over owns focus until closed"). Ordo's own passive
/// surfaces (the rail overlay, the shortcut reference) declare neither.
pub const MODAL_CONTEXTS: &[&str] = &["Modal", "Dialog", "Sheet"];

fn predicate(text_entry: bool) -> String {
    let mut predicate = PRESENTER_CONTEXT.to_owned();
    let text = TEXT_ENTRY_CONTEXTS.iter().filter(|_| text_entry);
    for context in text.chain(MODAL_CONTEXTS) {
        predicate.push_str(" && !");
        predicate.push_str(context);
    }
    predicate
}

/// `Ctrl+/` stays available in text fields (it is chorded, so it types
/// nothing) but never in a true modal: `Presenter && !Modal && !Dialog ...`.
pub fn reference_predicate() -> String {
    predicate(false)
}

/// Active only inside the Presenter and never while a text input or a modal
/// anywhere above focus declares its own context (`!X` is false if any
/// context on the focus stack matches `X`): `Presenter && !TextEntry && ...`.
pub fn binding_predicate() -> String {
    predicate(true)
}

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
    /// `Tab` / `Shift+Tab`: next or previous control (bound by `ui`).
    FocusNext,
    /// `Ctrl+=` / `Ctrl++`: larger Interface scale (bound by `ui`).
    ScaleUp,
    /// `Ctrl+-`: smaller Interface scale (bound by `ui`).
    ScaleDown,
    /// `Ctrl+0`: Interface scale back to 100% (bound by `ui`).
    ScaleReset,
    /// `Ctrl+Z`: editor Undo in an edit session.
    Undo,
    /// `Ctrl+Y`: editor Redo in an edit session.
    Redo,
    /// `Alt+Up`: moves the selected section up (the editor handles it).
    MoveUp,
    /// `Alt+Down`: moves the selected section down (the editor handles it).
    MoveDown,
}

/// The reference group an entry is listed under. Editing and the other
/// workspaces join when they register keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Keys that work everywhere in Ordo.
    Global,
    Presenter,
    /// Keys of an open editor's edit session.
    Editing,
}

impl Group {
    /// Every group, in the reference's fixed order (the reference moves the
    /// current workspace to the front).
    pub const ALL: [Group; 3] = [Group::Global, Group::Presenter, Group::Editing];

    pub const fn title(self) -> &'static str {
        match self {
            Group::Global => "Global",
            Group::Presenter => "Presenter",
            Group::Editing => "Editing",
        }
    }
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
    /// `Ctrl+Z`, `Ctrl+Y`, `Alt+Up`, `Alt+Down`: owned by an open editor's
    /// edit session; bound under [`EDIT_SESSION_CONTEXT`], never an Engine
    /// command and never reaching the Presenter.
    Editing,
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

const fn global(
    id: CommandId,
    keys: &'static [&'static str],
    keycaps: &'static str,
    label: &'static str,
) -> Entry {
    Entry {
        group: Group::Global,
        ..entry(id, keys, keycaps, label, Scope::Surface)
    }
}

const fn editing(
    id: CommandId,
    keys: &'static [&'static str],
    keycaps: &'static str,
    label: &'static str,
) -> Entry {
    Entry {
        group: Group::Editing,
        ..entry(id, keys, keycaps, label, Scope::Editing)
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
    global(
        CommandId::CloseSurface,
        &["escape"],
        "Esc",
        "Close the open panel",
    ),
    global(
        CommandId::CycleRegion,
        &["f6", "shift-f6"],
        "F6 / Shift+F6",
        "Next or previous region",
    ),
    global(
        CommandId::FocusNext,
        &["tab", "shift-tab"],
        "Tab / Shift+Tab",
        "Next or previous control",
    ),
    global(
        CommandId::ToggleReference,
        &["ctrl-/"],
        "Ctrl+/",
        "Keyboard shortcuts",
    ),
    global(
        CommandId::ScaleUp,
        &["ctrl-=", "ctrl-+"],
        "Ctrl+= / Ctrl++",
        "Increase Interface scale",
    ),
    global(
        CommandId::ScaleDown,
        &["ctrl--"],
        "Ctrl+-",
        "Decrease Interface scale",
    ),
    global(
        CommandId::ScaleReset,
        &["ctrl-0"],
        "Ctrl+0",
        "Reset Interface scale",
    ),
    editing(CommandId::Undo, &["ctrl-z"], "Ctrl+Z", "Undo"),
    editing(CommandId::Redo, &["ctrl-y"], "Ctrl+Y", "Redo"),
    editing(CommandId::MoveUp, &["alt-up"], "Alt+Up", "Move section up"),
    editing(
        CommandId::MoveDown,
        &["alt-down"],
        "Alt+Down",
        "Move section down",
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
        Scope::Surface | Scope::Editing => false,
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
        | CommandId::ToggleReference
        | CommandId::FocusNext
        | CommandId::ScaleUp
        | CommandId::ScaleDown
        | CommandId::ScaleReset
        | CommandId::Undo
        | CommandId::Redo
        | CommandId::MoveUp
        | CommandId::MoveDown => None,
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

/// The GPUI action an entry binds here, `None` for a key bound elsewhere
/// (`Esc`, and the keys `ui::key_bindings` owns).
fn action_for(id: CommandId) -> Option<Box<dyn Action>> {
    macro_rules! arms {
        ($($name:ident),*) => {
            match id {
                $(CommandId::$name => Some(Box::new($name) as Box<dyn Action>),)*
                CommandId::ToggleReference => Some(Box::new(ToggleReference) as Box<dyn Action>),
                CommandId::Undo => Some(Box::new(Undo) as Box<dyn Action>),
                CommandId::Redo => Some(Box::new(Redo) as Box<dyn Action>),
                CommandId::MoveUp => Some(Box::new(MoveUp) as Box<dyn Action>),
                CommandId::MoveDown => Some(Box::new(MoveDown) as Box<dyn Action>),
                CommandId::CloseSurface
                | CommandId::CycleRegion
                | CommandId::FocusNext
                | CommandId::ScaleUp
                | CommandId::ScaleDown
                | CommandId::ScaleReset => None,
            }
        };
    }
    for_each_bound_command!(arms)
}

/// Only the Editing bindings, for a host that has an edit session and no
/// Presenter (the edit-session lab).
pub fn editing_key_bindings() -> Vec<KeyBinding> {
    let editing: std::collections::BTreeSet<String> = ENTRIES
        .iter()
        .filter(|entry| entry.scope == Scope::Editing)
        .flat_map(|entry| entry.keys.iter().copied())
        .map(|keys| {
            gpui::Keystroke::parse(keys)
                .expect("every registered keystroke parses")
                .unparse()
        })
        .collect();
    key_bindings()
        .into_iter()
        .filter(|binding| {
            binding
                .keystrokes()
                .iter()
                .all(|keystroke| editing.contains(&keystroke.inner().unparse()))
        })
        .collect()
}

/// The GPUI bindings for every entry this module binds: the Output and
/// Operating keys under [`binding_predicate`], and `Ctrl+/` under
/// [`reference_predicate`].
pub fn key_bindings() -> Vec<KeyBinding> {
    let parse = |predicate: &str| {
        Rc::new(
            gpui::KeyBindingContextPredicate::parse(predicate)
                .expect("the binding predicate parses"),
        )
    };
    let single_key = parse(&binding_predicate());
    let reference = parse(&reference_predicate());
    let mut bindings = Vec::new();
    for entry in ENTRIES {
        let Some(action) = action_for(entry.id) else {
            continue;
        };
        let predicate = match entry.scope {
            Scope::Surface => reference.clone(),
            Scope::Editing => parse(editing_context(entry.id)),
            Scope::Output | Scope::Operating => single_key.clone(),
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
            gpui::KeyBindingContextPredicate::parse(&binding_predicate()).is_ok(),
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
    fn bindings_cover_every_entry_this_module_binds() {
        let bindings = key_bindings();
        let bound: BTreeSet<String> = bindings
            .iter()
            .flat_map(|b| b.keystrokes().iter().map(|k| k.inner().unparse()))
            .collect();
        let mut expected = BTreeSet::new();
        let mut count = 0;
        for entry in ENTRIES {
            let bound_here =
                entry.scope != Scope::Surface || entry.id == CommandId::ToggleReference;
            for keys in entry.keys {
                let keystroke = gpui::Keystroke::parse(keys).unwrap().unparse();
                if bound_here {
                    expected.insert(keystroke);
                    count += 1;
                } else {
                    assert!(!bound.contains(&keystroke), "{keys:?} is bound elsewhere");
                }
            }
        }
        assert_eq!(bound, expected);
        assert_eq!(bindings.len(), count);
    }

    #[test]
    fn single_keys_carry_the_text_entry_and_modal_guard_and_ctrl_slash_only_the_modal_one() {
        for binding in key_bindings() {
            let predicate = binding
                .predicate()
                .expect("a context predicate")
                .to_string();
            let keystroke = binding.keystrokes()[0].inner().unparse();
            let editing = ENTRIES
                .iter()
                .filter(|entry| entry.scope == Scope::Editing)
                .find(|entry| {
                    entry
                        .keys
                        .iter()
                        .any(|keys| gpui::Keystroke::parse(keys).unwrap().unparse() == keystroke)
                });
            assert_eq!(
                predicate,
                if let Some(entry) = editing {
                    editing_context(entry.id).to_owned()
                } else if keystroke == "ctrl-/" {
                    reference_predicate()
                } else {
                    binding_predicate()
                }
            );
        }
    }

    #[test]
    fn predicates_exclude_every_suspending_context() {
        let single = binding_predicate();
        let chorded = reference_predicate();
        for context in TEXT_ENTRY_CONTEXTS.iter().chain(MODAL_CONTEXTS) {
            let negated = format!("!{context}");
            assert!(
                single.split(" && ").any(|term| term == negated),
                "{context} missing from {single}"
            );
            assert_eq!(
                chorded.split(" && ").any(|term| term == negated),
                MODAL_CONTEXTS.contains(context),
                "{context} in {chorded}"
            );
        }
        // The literal context names `gpui-base` 0.7.1 declares, hard-coded so
        // the lists are checked against the library and not against
        // themselves (`src/` of the crate):
        //   input/base/state.rs  `Input`      combobox.rs    `Combobox`
        //   number_input.rs      `NumberInput` time_field.rs `TimeField`
        //   dialog.rs            `Dialog`      sheet.rs      `Sheet`
        for real in ["Input", "NumberInput", "Combobox", "TimeField"] {
            assert!(TEXT_ENTRY_CONTEXTS.contains(&real), "{real}");
        }
        for real in ["Dialog", "Sheet"] {
            assert!(MODAL_CONTEXTS.contains(&real), "{real}");
        }
        assert!(!single.contains("Root"));
    }

    /// `gpui-base`'s `OtpInput` declares no key context and reads digits raw,
    /// so a bound digit key would be stolen from (or leak into) it. No
    /// Presenter key may therefore be a digit.
    #[test]
    fn no_bound_key_is_a_digit() {
        for entry in ENTRIES {
            for keys in entry.keys {
                // Chords (`ctrl-0`) are not typed text; only bare digits matter.
                let stroke = gpui::Keystroke::parse(keys).unwrap();
                assert!(
                    stroke.modifiers.modified() || !stroke.key.chars().all(|c| c.is_ascii_digit()),
                    "{keys} of {:?} is a digit",
                    entry.id
                );
            }
        }
    }

    /// The keys `ui` binds are listed here so the reference shows them; this
    /// keeps the two from drifting in either direction, key by key and in what
    /// each key means.
    #[test]
    fn every_ui_key_is_listed_and_every_listed_ui_key_is_bound() {
        // The `ui` action each binding runs, as the Global entry that lists it.
        fn listed_as(action: &str) -> CommandId {
            match action {
                "NextRegion" | "PreviousRegion" => CommandId::CycleRegion,
                "FocusNext" | "FocusPrevious" => CommandId::FocusNext,
                "IncreaseInterfaceScale" => CommandId::ScaleUp,
                "DecreaseInterfaceScale" => CommandId::ScaleDown,
                "ResetInterfaceScale" => CommandId::ScaleReset,
                other => panic!("`ui` binds {other}, which the registry does not list"),
            }
        }
        let normal = |keys: &str| gpui::Keystroke::parse(keys).unwrap().unparse();
        let mut bound: std::collections::BTreeMap<String, BTreeSet<String>> = Default::default();
        for binding in ui::key_bindings() {
            let action = binding.action().name().rsplit("::").next().unwrap();
            let id = listed_as(action);
            let entry = entry_for(id);
            assert_eq!(entry.group, Group::Global, "{id:?}");
            for keystroke in binding.keystrokes() {
                let keys = keystroke.inner().unparse();
                assert!(
                    entry.keys.iter().any(|listed| normal(listed) == keys),
                    "{keys} runs {action} but {id:?} lists {:?}",
                    entry.keys
                );
                bound.entry(format!("{id:?}")).or_default().insert(keys);
            }
        }
        for entry in ENTRIES.iter().filter(|e| {
            e.group == Group::Global
                && !matches!(e.id, CommandId::ToggleReference | CommandId::CloseSurface)
        }) {
            let listed: BTreeSet<String> = entry.keys.iter().map(|k| normal(k)).collect();
            assert_eq!(
                bound.get(&format!("{:?}", entry.id)),
                Some(&listed),
                "{:?}",
                entry.id
            );
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
            assert!(!allows(Scope::Editing, context), "{context:?}");
        }
        for context in [TextEntry, Modal] {
            for scope in [
                Scope::Output,
                Scope::Operating,
                Scope::Surface,
                Scope::Editing,
            ] {
                assert!(!allows(scope, context), "{scope:?} in {context:?}");
            }
        }
    }

    #[test]
    fn surface_and_view_only_entries_send_no_engine_command() {
        for entry in ENTRIES {
            let command = command_for(entry.id);
            match entry.scope {
                Scope::Surface | Scope::Editing => assert!(command.is_none(), "{:?}", entry.id),
                _ if entry.id == CommandId::OpenOrdo => assert!(command.is_none()),
                _ => assert!(command.is_some(), "{:?}", entry.id),
            }
        }
    }

    /// Decision 17: the Editing keys are registry rows bound only under the
    /// edit-session context, so none can fire in the Presenter.
    #[test]
    fn undo_and_redo_keys_are_registry_rows() {
        for (id, key, keycaps) in [
            (CommandId::Undo, "ctrl-z", "Ctrl+Z"),
            (CommandId::Redo, "ctrl-y", "Ctrl+Y"),
            (CommandId::MoveUp, "alt-up", "Alt+Up"),
            (CommandId::MoveDown, "alt-down", "Alt+Down"),
        ] {
            let row = entry_for(id);
            assert_eq!(row.group, Group::Editing, "{id:?}");
            assert_eq!(row.scope, Scope::Editing, "{id:?}");
            assert_eq!(row.keys, &[key], "{id:?}");
            assert_eq!(row.keycaps, keycaps, "{id:?}");
            let binding = key_bindings()
                .into_iter()
                .find(|b| b.keystrokes()[0].inner().unparse() == key)
                .unwrap_or_else(|| panic!("{key} is bound"));
            assert_eq!(
                binding.predicate().unwrap().to_string(),
                editing_context(id),
                "{key} must not reach the Presenter"
            );
        }
        assert_eq!(Group::Editing.title(), "Editing");
        assert_eq!(editing_key_bindings().len(), 4);
    }

    #[test]
    fn every_command_id_has_one_row() {
        assert_eq!(entry_for(CommandId::TakeLive).keycaps, "Space");
        assert_eq!(entry_for(CommandId::ToggleReference).keys, &["ctrl-/"]);
    }
}
