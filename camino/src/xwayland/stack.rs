//! X's stacking order of the root window's children, as Smithay's callbacks show it (36).
//! Override-redirect windows are drawn in this order among themselves.
//!
//! A `configure_notify` names the sibling just below the window, which may be a managed
//! window's frame, so the order among override-redirect windows alone can't be kept without
//! knowing where the frames are too. Camino never asks X, whose connection is Smithay's
//! (38 adds Camino's own), so it follows every root child it hears about instead.

use smithay::xwayland::xwm::X11Window;

/// One of the root's children: a client's window, or the frame Smithay reparented a managed
/// window into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Child {
    window: X11Window,
    frame: Option<X11Window>,
}

impl Child {
    /// The id X names it by as a sibling.
    fn sibling_id(&self) -> X11Window {
        self.frame.unwrap_or(self.window)
    }
}

/// The root's children, bottom to top, each known by its client window's id. Smithay's own
/// windows aren't in it; they were made before any client's, so they're at the bottom.
#[derive(Debug, Default)]
pub(crate) struct RootStack(Vec<Child>);

impl RootStack {
    /// A window was created on the root, which puts it on top.
    pub(crate) fn created(&mut self, window: X11Window) {
        self.push(Child {
            window,
            frame: None,
        });
    }

    /// Smithay reparented a managed window into `frame`, which it made on top.
    pub(crate) fn framed(&mut self, window: X11Window, frame: X11Window) {
        self.push(Child {
            window,
            frame: Some(frame),
        });
    }

    /// Smithay reparented an unmapped managed window back onto the root, on top, and
    /// destroyed its frame.
    pub(crate) fn unframed(&mut self, window: X11Window) {
        if self
            .index(window)
            .is_some_and(|i| self.0[i].frame.is_some())
        {
            self.created(window);
        }
    }

    pub(crate) fn destroyed(&mut self, window: X11Window) {
        self.0.retain(|child| child.window != window);
    }

    /// `window` was restacked just above the sibling `above`, or at the bottom. A
    /// sibling Camino doesn't know is one of Smithay's, at the bottom.
    pub(crate) fn restacked(&mut self, window: X11Window, above: Option<X11Window>) {
        let Some(index) = self.index(window) else {
            return;
        };
        let child = self.0.remove(index);
        let at = above
            .and_then(|above| self.0.iter().position(|c| c.sibling_id() == above))
            .map_or(0, |below| below + 1);
        self.0.insert(at, child);
    }

    /// Where `window` is, counting from the bottom.
    pub(crate) fn index(&self, window: X11Window) -> Option<usize> {
        self.0.iter().position(|child| child.window == window)
    }

    fn push(&mut self, child: Child) {
        self.destroyed(child.window);
        self.0.push(child);
    }
}

#[cfg(test)]
mod tests {
    use super::RootStack;

    fn order(stack: &RootStack, windows: &[u32]) -> Vec<u32> {
        let mut windows = windows.to_vec();
        windows.sort_by_key(|w| stack.index(*w));
        windows
    }

    #[test]
    fn a_window_restacked_above_a_frame_goes_just_above_it() {
        let mut stack = RootStack::default();
        stack.created(1);
        stack.framed(1, 100);
        stack.created(2);
        stack.created(3);
        // 2 lowered below 3, which puts it just above 1's frame.
        stack.restacked(2, Some(3));
        stack.restacked(2, Some(100));

        assert_eq!(order(&stack, &[1, 2, 3]), [1, 2, 3]);
    }

    #[test]
    fn a_window_restacked_above_nothing_or_an_unknown_sibling_goes_to_the_bottom() {
        let mut stack = RootStack::default();
        stack.created(1);
        stack.created(2);
        stack.created(3);
        stack.restacked(3, None);
        assert_eq!(order(&stack, &[1, 2, 3]), [3, 1, 2]);

        stack.restacked(2, Some(999));
        assert_eq!(order(&stack, &[1, 2, 3]), [2, 3, 1]);
    }

    #[test]
    fn a_frame_is_named_by_its_own_id_and_unframing_puts_the_window_on_top() {
        let mut stack = RootStack::default();
        stack.created(1);
        stack.created(2);
        stack.framed(1, 100);
        // The frame's id, not the client window's, is what X names as a sibling.
        stack.restacked(2, Some(1));
        assert_eq!(order(&stack, &[1, 2]), [2, 1]);
        stack.restacked(2, Some(100));
        assert_eq!(order(&stack, &[1, 2]), [1, 2]);

        stack.restacked(1, Some(2));
        stack.unframed(1);
        stack.restacked(2, Some(1));
        assert_eq!(order(&stack, &[1, 2]), [1, 2]);
    }
}
