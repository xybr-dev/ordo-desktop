//! The temporary surfaces open over the Presenter, in the order they opened.
//!
//! `Esc` addresses the topmost one and nothing else: it closes that surface and
//! leaves the Engine, Program and Preview alone. A pinned rail is not a
//! temporary surface and never appears here. Pure data, no GPUI.

/// A temporary surface that `Esc` can close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The Ordo rail opened as an overlay (not pinned).
    RailOverlay,
    /// The Keyboard shortcuts reference; T1.5d opens it.
    #[allow(dead_code, reason = "T1.5d opens the shortcut reference")]
    ShortcutReference,
}

/// Open surfaces, oldest first; the last is topmost.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurfaceStack(Vec<Surface>);

impl SurfaceStack {
    /// Opens `surface` on top. A surface already open moves to the top.
    pub fn open(&mut self, surface: Surface) {
        self.close(surface);
        self.0.push(surface);
    }

    /// Closes `surface` wherever it is; false if it was not open.
    pub fn close(&mut self, surface: Surface) -> bool {
        let before = self.0.len();
        self.0.retain(|open| *open != surface);
        self.0.len() != before
    }

    pub fn contains(&self, surface: Surface) -> bool {
        self.0.contains(&surface)
    }

    pub fn topmost(&self) -> Option<Surface> {
        self.0.last().copied()
    }

    /// Closes and returns the topmost surface.
    pub fn close_topmost(&mut self) -> Option<Surface> {
        self.0.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_closes_only_the_topmost_surface() {
        let mut stack = SurfaceStack::default();
        assert_eq!(stack.close_topmost(), None, "nothing open, nothing closes");
        stack.open(Surface::RailOverlay);
        stack.open(Surface::ShortcutReference);
        assert_eq!(stack.topmost(), Some(Surface::ShortcutReference));
        assert_eq!(stack.close_topmost(), Some(Surface::ShortcutReference));
        assert!(stack.contains(Surface::RailOverlay));
        assert_eq!(stack.close_topmost(), Some(Surface::RailOverlay));
        assert_eq!(stack.topmost(), None);
    }

    #[test]
    fn reopening_moves_a_surface_to_the_top_without_duplicating_it() {
        let mut stack = SurfaceStack::default();
        stack.open(Surface::RailOverlay);
        stack.open(Surface::ShortcutReference);
        stack.open(Surface::RailOverlay);
        assert_eq!(stack.topmost(), Some(Surface::RailOverlay));
        assert!(stack.close(Surface::RailOverlay));
        assert!(!stack.close(Surface::RailOverlay));
        assert_eq!(stack.topmost(), Some(Surface::ShortcutReference));
    }
}
