//! The **Quit Ordo?** confirmation, shown when the operator window is asked
//! to close (Alt+F4, the window's close button) instead of closing at once.
//!
//! The dialog is built here; whoever owns the window's overlay layer opens it
//! (the shell in Preparation, the Presenter in Presenter Mode, because only
//! one of the two layers is mounted at a time).

use gpui::{App, FocusHandle, Window};
use ui::overlay::Overlay;

pub const TITLE: &str = "Quit Ordo?";
pub const QUIT: &str = "Quit";
pub const CANCEL: &str = "Cancel";

/// With output enabled, quitting also takes the congregation display down.
const OUTPUT_NOTE: &str = "Program goes black and the congregation display closes.";

/// **Quit** is the primary action and holds initial focus (`quit`), so Enter
/// quits; Esc, the scrim and **Cancel** keep Ordo open.
pub fn confirmation(
    quit: &FocusHandle,
    output_enabled: bool,
    on_quit: impl Fn(&mut Window, &mut App) + 'static,
) -> Overlay {
    let dialog = Overlay::dialog(TITLE);
    let dialog = if output_enabled {
        dialog.description(OUTPUT_NOTE)
    } else {
        dialog
    };
    dialog
        .primary(QUIT, on_quit)
        .primary_focus(quit)
        .secondary(CANCEL, |_, _| {})
}
