//! Using a frame with the pointer, as Marco's `frames.c` does (07, 31): buttons that act
//! when clicked and released over them, the titlebar's drag, double-click and other
//! clicks, and resizing from the borders.

use caminito_policy::{PolicyWindow, WindowId};
use caminito_theme::{ButtonFunction, ButtonState, ButtonStates};
use smithay::backend::input::InputTime;
use smithay::utils::{Logical, Point, Serial};

use super::grab::{FrameHold, GrabKind, start_data};
use super::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT};
use crate::decoration::control::FrameControl;
use crate::settings::translate::TitlebarAction;
use crate::state::{Caminito, SEAT, State};

/// How far apart, in logical pixels, a double-click's presses may be: GTK's default
/// `gtk-double-click-distance`, which Marco's frames got their double-clicks from.
const DOUBLE_CLICK_DISTANCE: f64 = 5.;

/// Where the pointer is on the frames, and what it's doing with them.
#[derive(Debug, Default)]
pub struct FramePointer {
    /// The window and control the pointer is on, when it's on a frame.
    pub hover: Option<(WindowId, FrameControl)>,
    /// The frame button being clicked.
    pub pressed: Option<(WindowId, ButtonFunction)>,
    /// The last left press on a frame, which a second one may make a double-click.
    last_press: Option<Press>,
    /// How far a scroll on a titlebar has gone since its last whole click of a wheel, in
    /// 120ths of one (162).
    pub(super) scrolled: f64,
}

#[derive(Debug, Clone, Copy)]
struct Press {
    window: WindowId,
    location: Point<f64, Logical>,
    time: InputTime,
}

impl Caminito {
    /// Whether a window is showing a frame, for the pointer to use.
    fn is_framed(&self, window: WindowId) -> bool {
        self.policy
            .window(window)
            .is_some_and(|mapped| mapped.shows_frame())
    }

    /// Whether a left press on `window`'s frame is the second of a double-click, as GDK's
    /// `GDK_2BUTTON_PRESS` is: soon enough after the first and near it. A third press
    /// starts over.
    fn double_click(
        &mut self,
        window: WindowId,
        location: Point<f64, Logical>,
        time: InputTime,
    ) -> bool {
        let limit = self.settings.mouse.double_click_time;
        let double = self.frame_pointer.last_press.is_some_and(|last| {
            let apart = location - last.location;
            let elapsed = time.micros().saturating_sub(last.time.micros());
            last.window == window
                && elapsed <= limit.as_micros() as u64
                && apart.x.abs() <= DOUBLE_CLICK_DISTANCE
                && apart.y.abs() <= DOUBLE_CLICK_DISTANCE
        });
        self.frame_pointer.last_press = (!double).then_some(Press {
            window,
            location,
            time,
        });
        double
    }

    /// Draws each frame's buttons in the state the pointer leaves them: the button under
    /// the pointer prelit, or pressed while it's the one being clicked. While a button is
    /// clicked, no other one lights up.
    pub(crate) fn update_button_states(&mut self) {
        let pointer = &self.frame_pointer;
        let lit_button = match (pointer.hover, pointer.pressed) {
            _ if self.window_grab.is_some() => None,
            (Some((id, FrameControl::Button(function))), None) => {
                Some((id, function, ButtonState::Prelight))
            }
            (Some((id, FrameControl::Button(function))), Some(pressed))
                if pressed == (id, function) =>
            {
                Some((id, function, ButtonState::Pressed))
            }
            _ => None,
        };
        let ids: Vec<WindowId> = self.policy.windows().map(|mapped| mapped.id()).collect();
        for id in ids {
            let mut states = ButtonStates::default();
            if let Some((window, function, state)) = lit_button
                && window == id
            {
                states.set(function, state);
            }
            let mapped = self.policy.window_mut(id).unwrap();
            mapped.frame_mut().set_button_states(states);
        }
    }
}

impl State {
    /// A press on a window's frame, as Marco's `meta_frames_button_press_event` handles
    /// it. The client never sees it.
    pub(super) fn frame_press(
        &mut self,
        window: WindowId,
        control: FrameControl,
        button: u32,
        location: Point<f64, Logical>,
        serial: Serial,
        time: InputTime,
    ) {
        let caminito = &mut self.caminito;
        if !caminito.is_framed(window) {
            return;
        }
        let double = button == BTN_LEFT && caminito.double_click(window, location, time);

        // A left press focuses, except on the buttons that act on the window.
        let acts_on_window = matches!(
            control,
            FrameControl::Button(
                ButtonFunction::Minimize | ButtonFunction::Close | ButtonFunction::Maximize
            )
        );
        if button == BTN_LEFT && !acts_on_window {
            let now = caminito.now();
            caminito.policy.click(SEAT, window, now);
            self.update_keyboard_focus();
        }

        let frames = &self.caminito.settings.frames;
        let hold = match (control, button) {
            (FrameControl::Title, BTN_LEFT) if double => {
                self.titlebar_action(window, frames.double_click);
                FrameHold::Held
            }
            // The maximise button takes any button; the others only the left one.
            (FrameControl::Button(function @ ButtonFunction::Maximize), _)
            | (
                FrameControl::Button(
                    function @ (ButtonFunction::Minimize
                    | ButtonFunction::Close
                    | ButtonFunction::Shade
                    | ButtonFunction::Unshade
                    | ButtonFunction::Above
                    | ButtonFunction::Unabove
                    | ButtonFunction::Stick
                    | ButtonFunction::Unstick
                    | ButtonFunction::Menu),
                ),
                BTN_LEFT,
            ) => FrameHold::Button { window, function },
            (FrameControl::Resize(edges), BTN_LEFT) => {
                let start_data = start_data(button, location);
                self.begin_window_grab(window, GrabKind::Resize(edges), start_data, serial);
                return;
            }
            (FrameControl::Title, BTN_LEFT) => FrameHold::Title {
                window,
                // Marco's `META_WINDOW_ALLOWS_MOVE` (window-private.h:440).
                can_move: !self.caminito.policy.is_fullscreen(window),
            },
            (_, BTN_MIDDLE) => {
                self.titlebar_action(window, frames.middle_click);
                FrameHold::Held
            }
            (_, BTN_RIGHT) => {
                self.titlebar_action(window, frames.right_click);
                FrameHold::Held
            }
            _ => return,
        };
        self.begin_frame_grab(hold, start_data(button, location), serial);
    }

    /// The button a frame button was clicked with was released. It acts if the pointer is
    /// still over the button, as in Marco's `meta_frames_button_release_event`.
    pub(super) fn frame_button_released(
        &mut self,
        window: WindowId,
        function: ButtonFunction,
        button: u32,
    ) {
        let caminito = &mut self.caminito;
        if caminito.frame_pointer.hover != Some((window, FrameControl::Button(function))) {
            return;
        }
        let policy = &mut caminito.policy;
        match function {
            ButtonFunction::Close => policy.close(window),
            ButtonFunction::Minimize => policy.minimize(window),
            ButtonFunction::Maximize => {
                let now = caminito.now();
                caminito.policy.click(SEAT, window, now);
                // Unmaximise takes any button. The middle and right buttons maximise one
                // way, as Marco's do.
                match button {
                    _ if caminito.policy.is_maximized(window) => self.toggle_maximized(window),
                    BTN_MIDDLE => self.toggle_maximized_vertically(window),
                    BTN_RIGHT => self.toggle_maximized_horizontally(window),
                    _ => self.toggle_maximized(window),
                }
            }
            ButtonFunction::Shade => policy.set_shaded(window, true),
            ButtonFunction::Unshade => policy.set_shaded(window, false),
            // As Marco's `meta_frames_button_release_event` (155).
            ButtonFunction::Above => policy.set_above(window, true),
            ButtonFunction::Unabove => policy.set_above(window, false),
            ButtonFunction::Stick => policy.set_sticky(window, true),
            ButtonFunction::Unstick => policy.set_sticky(window, false),
            // Under the button's bottom left corner, as Marco's
            // `meta_frames_button_press_event` puts it (frames.c:1826-1847), though Marco
            // opens it at the press (161).
            ButtonFunction::Menu => {
                let below = policy.window(window).and_then(|mapped| {
                    let rect = mapped.frame().button_rect(function)?;
                    Some(policy.geometry(window)?.loc + rect.loc + Point::from((0, rect.size.h)))
                });
                if let Some(below) = below {
                    let time = InputTime::from_micros(caminito.now().as_micros() as u64);
                    self.open_window_menu(window, below, time);
                }
            }
            ButtonFunction::Appmenu => {
                tracing::debug!("the {function:?} button does nothing yet");
            }
        }
        self.update_keyboard_focus();
        self.caminito.queue_redraw_all();
    }

    /// Runs one of the `action-*-titlebar` actions on a window (`meta_frame_titlebar_event`,
    /// frames.c:1480-1650). Policy's setters refuse a state the window has no function for
    /// (177).
    pub(super) fn titlebar_action(&mut self, window: WindowId, action: TitlebarAction) {
        let caminito = &mut self.caminito;
        if !caminito.is_framed(window) {
            return;
        }
        let now = caminito.now();
        let policy = &mut caminito.policy;
        let functions = policy.functions(window);
        match action {
            TitlebarAction::ToggleShade => policy.set_shaded(window, !policy.is_shaded(window)),
            TitlebarAction::Shade => policy.set_shaded(window, true),
            TitlebarAction::Unshade => policy.set_shaded(window, false),
            TitlebarAction::ToggleMaximize => self.toggle_maximized(window),
            // Marco's frame asks the maximise function for one way too (frames.c:1527-1549),
            // where its keybindings, and so Policy's setters, ask only the resize function.
            TitlebarAction::ToggleMaximizeHorizontally if functions.maximize => {
                self.toggle_maximized_horizontally(window);
            }
            TitlebarAction::ToggleMaximizeVertically if functions.maximize => {
                self.toggle_maximized_vertically(window);
            }
            TitlebarAction::Minimize => policy.minimize(window),
            // A frame that's a border only has no `META_FRAME_ALLOWS_DELETE`
            // (`meta_frame_get_flags`, frame.c:260-281).
            TitlebarAction::Close if !functions.border_only => policy.close(window),
            TitlebarAction::Lower => policy.lower(SEAT, window, now),
            TitlebarAction::Raise => policy.raise(window),
            // As Marco's `meta_frame_titlebar_event` toggles them (frames.c:1614-1642)
            // (155).
            TitlebarAction::ToggleAbove => policy.set_above(window, !policy.is_above(window)),
            TitlebarAction::ToggleStick => policy.set_sticky(window, !policy.is_sticky(window)),
            // At the pointer, as Marco's `meta_frame_titlebar_event` opens it
            // (frames.c:1570-1577) (161).
            TitlebarAction::Menu => {
                let pointer = caminito.seat.get_pointer().unwrap();
                let at = pointer.current_location().to_i32_floor();
                let time = InputTime::from_micros(now.as_micros() as u64);
                self.open_window_menu(window, at, time);
            }
            _ => {}
        }
        self.update_keyboard_focus();
        self.caminito.queue_redraw_all();
    }

    /// Maximises the window both ways, or unmaximises it both ways if it's maximised both
    /// ways.
    fn toggle_maximized(&mut self, window: WindowId) {
        let policy = &mut self.caminito.policy;
        policy.set_maximized(window, !policy.is_maximized(window));
    }

    fn toggle_maximized_horizontally(&mut self, window: WindowId) {
        let policy = &mut self.caminito.policy;
        policy.set_maximized_horizontally(window, !policy.is_maximized_horizontally(window));
    }

    fn toggle_maximized_vertically(&mut self, window: WindowId) {
        let policy = &mut self.caminito.policy;
        policy.set_maximized_vertically(window, !policy.is_maximized_vertically(window));
    }
}
