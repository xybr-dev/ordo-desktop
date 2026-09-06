//! Ordo-owned GPUI primitives and application-shell views.

pub mod accessibility;
pub mod assets;
pub mod environment;
/// Development-only scenario host; `ui-lab` enables it, operator builds do not.
#[cfg(any(test, feature = "lab"))]
pub mod lab;
pub mod overlay;
pub mod primitives;
pub mod scroll;
pub mod tokens;

use std::borrow::Cow;

use environment::ResolvedAppearance;
use gpui::{App, KeyBinding, WindowAppearance, actions};

// The actions `ui` itself binds and answers. Application navigation lives with
// the shell that owns it; these are the ones a primitive or the region registry
// has to handle, plus the Interface scale keys `init` binds.
actions!(
    ordo_ui,
    [
        IncreaseInterfaceScale,
        DecreaseInterfaceScale,
        ResetInterfaceScale,
        FocusNext,
        FocusPrevious,
        NextRegion,
        PreviousRegion
    ]
);

pub fn init(cx: &mut App) -> Result<(), String> {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(assets::ATKINSON_HYPERLEGIBLE_NEXT),
            Cow::Borrowed(assets::NOTO_SANS),
        ])
        .map_err(|error| error.to_string())?;
    // Infrastructure for the adopted base input behavior: it registers the
    // native editing, selection, and clipboard bindings a Field depends on.
    // `docs/qualification/GPUI-BASE-T0.3.md` bounds this to exactly that.
    gpui_base::init(cx);
    cx.bind_keys(key_bindings());
    Ok(())
}

/// `Tab` is not bound by GPUI and the pin binds it only inside a text input,
/// where a single-line Field propagates it straight back out. Ordo therefore
/// owns the two focus-order keys, which is what lets a modal surface contain
/// them; `DESIGN.md`'s `F6` region cycle joins them with the region registry.
pub(crate) fn key_bindings() -> [KeyBinding; 8] {
    [
        KeyBinding::new("f6", NextRegion, None),
        KeyBinding::new("shift-f6", PreviousRegion, None),
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrevious, None),
        KeyBinding::new("ctrl-=", IncreaseInterfaceScale, None),
        KeyBinding::new("ctrl-+", IncreaseInterfaceScale, None),
        KeyBinding::new("ctrl--", DecreaseInterfaceScale, None),
        KeyBinding::new("ctrl-0", ResetInterfaceScale, None),
    ]
}

/// The half of live operating-system appearance following the pinned GPUI
/// lets a test drive; see `docs/qualification/GPUI-BASE-T0.3.md`.
pub fn resolved_appearance(appearance: WindowAppearance) -> ResolvedAppearance {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => ResolvedAppearance::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => ResolvedAppearance::Dark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ctrl-shift-1` never fired on Linux: xkb reports the shifted keysym, so
    /// the key arrives as `!` and GPUI then drops `shift` for a non-letter, and
    /// no binding written with `shift` plus a non-letter can ever match.
    #[test]
    fn no_binding_asks_for_shift_with_a_key_shift_already_rewrites() {
        for binding in key_bindings().iter().chain(lab::key_bindings().iter()) {
            for keystroke in binding.keystrokes() {
                let keystroke = keystroke.inner();
                let unreachable = keystroke.modifiers.shift
                    && keystroke.key.chars().count() == 1
                    && keystroke.key.to_lowercase() == keystroke.key.to_uppercase();
                assert!(
                    !unreachable,
                    "{keystroke:?} can never be typed: the platform reports the shifted key instead"
                );
            }
        }
    }

    /// The half of live OS-theme following that the pinned GPUI revision lets us
    /// automate; `docs/qualification/GPUI-BASE-T0.3.md` records why the observer
    /// itself cannot be driven from a test.
    #[test]
    fn every_window_appearance_resolves_to_an_operator_appearance() {
        assert_eq!(
            resolved_appearance(WindowAppearance::Light),
            ResolvedAppearance::Light
        );
        assert_eq!(
            resolved_appearance(WindowAppearance::VibrantLight),
            ResolvedAppearance::Light
        );
        assert_eq!(
            resolved_appearance(WindowAppearance::Dark),
            ResolvedAppearance::Dark
        );
        assert_eq!(
            resolved_appearance(WindowAppearance::VibrantDark),
            ResolvedAppearance::Dark
        );
    }
}
