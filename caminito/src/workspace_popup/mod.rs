//! The Workspace popup (152): Marco's popup of the Workspace grid, which a switch or a move
//! to the workspace beside shows while the binding's modifiers are held
//! (`handle_workspace_switch_or_move`, keybindings.c:3505-3586). Policy holds that it's
//! open, and whether a move opened it, since it's the seat's Interaction (187): Policy
//! opens it only when the seat has none, and nothing else opens while it shows. Policy has
//! no selection for it: the selected workspace is the active one, since each press switches
//! at once, as in Marco. Here are when it shows and closes, the modifiers that hold it
//! open, where it is, and what it's drawn from. `layout` has its rectangles, `draw` draws
//! it, and `scene` puts it on the output.

pub(crate) mod draw;
pub(crate) mod layout;

use caminito_policy::{Action, Interaction, default_workspace_name};
use caminito_theme::{PaletteColors, Title, Variant, pango};
use smithay::backend::input::{ButtonState, InputTime};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::input::keyboard::{FilterResult, Keycode, KeysymHandle, ModifiersState};
use smithay::output::Output;
use smithay::utils::{Logical, Rectangle, Size};

use self::draw::Workspace;
use self::layout::Layout;
use crate::input::bindings::Binding;
use crate::input::{Bound, press_keysyms};
use crate::output::output_geometry;
use crate::state::{Caminito, SEAT, State};
use crate::switcher::Holding;

/// The open Workspace popup, on the output it appeared on. It doesn't follow the pointer
/// to another. Policy has whether a move opened it.
pub(crate) struct WorkspacePopup {
    /// The modifiers of the binding that opened it. Letting go of one closes it.
    holding: Holding,
    /// The output it's on.
    pub(crate) output: Output,
    /// Its rectangles there, as last laid out.
    pub(crate) layout: Layout,
    /// The popup as last drawn, unless cairo couldn't draw it.
    pub(crate) chrome: Option<MemoryRenderBuffer>,
    /// What it was drawn from, to draw it again only when that changes.
    drawn: Option<Drawn>,
}

/// Everything the popup is drawn from.
#[derive(PartialEq)]
struct Drawn {
    layout: Layout,
    selected: usize,
    names: Vec<String>,
    /// Each workspace's windows on the output, from its corner, bottom to top.
    windows: Vec<Vec<Rectangle<i32, Logical>>>,
    output: Size<i32, Logical>,
    scale: i32,
    colors: PaletteColors,
    font: String,
    /// The frames' generation, which counts the changes to the font rendering.
    generation: u64,
}

/// The two actions that go one way in the Workspace grid, and which of them a binding is.
struct Beside {
    switch: Action,
    move_window: Action,
    /// Whether the binding is the move.
    is_move: bool,
}

impl Beside {
    /// What `action` is, if it's one of the eight to the workspace beside. Those alone show
    /// the popup: a switch or a move by number, and `switch-to-workspace-prev`, show none
    /// in Marco (keybindings.c:2360-2393, 3418-3440).
    fn of(action: Action) -> Option<Self> {
        use Action::{
            MoveToWorkspaceDown, MoveToWorkspaceLeft, MoveToWorkspaceRight, MoveToWorkspaceUp,
            SwitchToWorkspaceDown, SwitchToWorkspaceLeft, SwitchToWorkspaceRight,
            SwitchToWorkspaceUp,
        };
        let (switch, move_window) = match action {
            SwitchToWorkspaceLeft | MoveToWorkspaceLeft => {
                (SwitchToWorkspaceLeft, MoveToWorkspaceLeft)
            }
            SwitchToWorkspaceRight | MoveToWorkspaceRight => {
                (SwitchToWorkspaceRight, MoveToWorkspaceRight)
            }
            SwitchToWorkspaceUp | MoveToWorkspaceUp => (SwitchToWorkspaceUp, MoveToWorkspaceUp),
            SwitchToWorkspaceDown | MoveToWorkspaceDown => {
                (SwitchToWorkspaceDown, MoveToWorkspaceDown)
            }
            _ => return None,
        };
        Some(Self {
            switch,
            move_window,
            is_move: action == move_window,
        })
    }
}

impl State {
    /// A pointer button. Returns whether its press was the popup's, and so is no client's:
    /// a press closes it, as a press ends Marco's grab (display.c:1859-1903), but that of a
    /// button with a binding to the workspace beside, which steps it as such a key does
    /// (184). The release of such a press is swallowed with the Window switcher's.
    pub(crate) fn workspace_popup_button(
        &mut self,
        button: u32,
        state: ButtonState,
        time: InputTime,
    ) -> bool {
        let caminito = &mut self.caminito;
        if state == ButtonState::Released || caminito.workspace_popup.is_none() {
            return false;
        }
        caminito.suppressed_buttons.insert(button);
        let mods = caminito.seat.get_keyboard().unwrap().modifier_state();
        let step = match caminito.bound_to_button(button, &mods) {
            Some((_, Binding::Policy(action))) => caminito.workspace_popup_step(action),
            _ => None,
        };
        match step {
            Some(action) => self.run_action(action, time),
            None => caminito.close_workspace_popup(),
        }
        true
    }
}

impl Caminito {
    /// Whether a binding's `action`, about to run, shows the popup: a switch or a move to
    /// the workspace beside, with more than one workspace, as in Marco
    /// (keybindings.c:3516-3518), and for a move a window that can be moved. It shows
    /// when the move goes nowhere too, at an edge of a grid that doesn't wrap, as
    /// Marco's does. It shows over no other Interaction, one the action is about to end
    /// among them: a switch closes the Window switcher and the Window menu.
    pub(crate) fn workspace_popup_shows_for(&self, action: Action) -> bool {
        let Some(beside) = Beside::of(action) else {
            return false;
        };
        let policy = &self.policy;
        let movable = || {
            let focus = policy.focus(SEAT);
            focus.is_some_and(|id| !policy.is_desktop_or_dock(id))
        };
        let interaction = policy.interaction(SEAT);
        policy.workspace_count() > 1
            && matches!(interaction, None | Some(Interaction::WorkspacePopup))
            && !self.workspace_popup_gives_way()
            && (!beside.is_move || movable())
    }

    /// What the popup gives way to that Policy doesn't hold: the session lock, which is no
    /// Interaction, and the pointer held for a move or resize, which outlasts one that
    /// Policy ended itself until the pointer next moves or its button goes up.
    fn workspace_popup_gives_way(&self) -> bool {
        self.lock.is_locked() || self.window_grab.is_some()
    }

    /// A binding's `action` ran with `mods` down, and shows the popup: it appears at once,
    /// as Marco's does, on the output the pointer is on, unless Policy refuses it for the
    /// seat's Interaction. An open popup stays as it is. A binding with no modifier shows
    /// none, since no release would close it; Marco ends such a grab as it begins
    /// (keybindings.c:3541-3552).
    pub(crate) fn show_workspace_popup(&mut self, action: Action, mods: &ModifiersState) {
        let holding = Holding::with_shift_of(mods);
        let Some(beside) = Beside::of(action) else {
            return;
        };
        if self.workspace_popup.is_none()
            && !holding.is_empty()
            && let Some(output) = self.popup_output()
            && self.policy.open_workspace_popup(beside.is_move)
        {
            let layout = self.workspace_popup_layout(&output);
            self.workspace_popup = Some(WorkspacePopup {
                holding,
                output,
                layout,
                chrome: None,
                drawn: None,
            });
        }
        self.refresh_workspace_popup();
    }

    /// What a binding's `action` does while the popup shows: the switch its way, or the
    /// move if a move opened the popup, whichever of the two the key is bound to, as Marco
    /// looks the key up with the modifiers the popup opened with (keybindings.c:2828-2882).
    /// `None` for any other action.
    pub(crate) fn workspace_popup_step(&self, action: Action) -> Option<Action> {
        let popup = self.policy.workspace_popup()?;
        let beside = Beside::of(action)?;
        Some(if popup.moves {
            beside.move_window
        } else {
            beside.switch
        })
    }

    /// A key went up, leaving `mods` down: the popup closes once the modifiers that hold
    /// it open aren't all down.
    pub(crate) fn workspace_popup_key_released(&mut self, mods: &ModifiersState) {
        let held = |popup: &WorkspacePopup| popup.holding.is_held_in(mods);
        if !self.workspace_popup.as_ref().is_none_or(held) {
            self.close_workspace_popup();
        }
    }

    /// A key went down while the Workspace popup may show. Returns what the keyboard path
    /// does with it, or `None` if the popup is closed and the key is the other stages'. The
    /// popup has the keyboard while it shows, as Marco's grab has
    /// (`process_workspace_switch_grab`, keybindings.c:2760-2899): a binding to the
    /// workspace beside steps it, and any other key closes it and goes no further. A
    /// modifier goes on to the client, as with the Window switcher open.
    pub(crate) fn workspace_popup_key_pressed(
        &mut self,
        keycode: Keycode,
        mods: &ModifiersState,
        keysym: &KeysymHandle<'_>,
    ) -> Option<FilterResult<Option<Bound>>> {
        self.workspace_popup.as_ref()?;
        if keysym.modified_sym().is_modifier_key() {
            return Some(FilterResult::Forward);
        }
        self.suppressed_keys.insert(keycode);
        let bindings = &self.settings.bindings;
        let step = match bindings.binding_for(mods, &press_keysyms(keysym)) {
            Some(Binding::Policy(action)) => self.workspace_popup_step(action),
            _ => None,
        };
        if step.is_none() {
            self.close_workspace_popup();
        }
        Some(FilterResult::Intercept(step.map(Bound::Action)))
    }

    /// Takes the popup off the outputs, if it shows, and tells Policy that it closed.
    pub(crate) fn close_workspace_popup(&mut self) {
        self.policy.close_workspace_popup();
        if self.workspace_popup.take().is_some() {
            self.queue_redraw_all();
        }
    }

    /// Brings the open popup up to date, in the refresh pass: closes it for what it gives
    /// way to, moves it off an output that's gone, lays it out again, and draws it again
    /// if anything it's drawn from changed.
    pub(crate) fn refresh_workspace_popup(&mut self) {
        let Some(shown) = self.workspace_popup.as_ref().map(|popup| &popup.output) else {
            return;
        };
        // The popup's output was unplugged or turned off under it.
        let output = if self.outputs.contains(shown) {
            Some(shown.clone())
        } else {
            self.popup_output()
        };
        let Some(output) = output.filter(|_| !self.workspace_popup_gives_way()) else {
            self.close_workspace_popup();
            return;
        };
        let drawn = self.workspace_popup_inputs(&output);
        let popup = self.workspace_popup.as_ref();
        if popup.is_some_and(|popup| popup.output == output && popup.drawn.as_ref() == Some(&drawn))
        {
            return;
        }
        let chrome = self.draw_workspace_popup(&drawn);
        if let Some(popup) = &mut self.workspace_popup {
            popup.output = output;
            popup.layout = drawn.layout.clone();
            popup.chrome = chrome;
            popup.drawn = Some(drawn);
        }
        self.queue_redraw_all();
    }

    /// The popup's rectangles on `output`, for the Workspace grid as it is now.
    fn workspace_popup_layout(&self, output: &Output) -> Layout {
        let policy = &self.policy;
        let size = output_geometry(output).size;
        Layout::new(&policy.workspace_grid(), policy.workspace_count(), size)
    }

    /// What the popup is drawn from on `output`, now.
    fn workspace_popup_inputs(&self, output: &Output) -> Drawn {
        let policy = &self.policy;
        let geometry = output_geometry(output);
        let active = policy.active_workspace();
        let names = policy.workspace_names();
        // A workspace's windows as Marco's popup has them (tabpopup.c:1151-1180): those
        // showing there, without the desktop and the docks, which skip its pager, and
        // without a window on every workspace but on the active one.
        let windows = |workspace: usize| {
            let ids = policy.stacking(workspace).into_iter();
            let shown = ids.filter(|id| {
                !policy.is_minimized(*id)
                    && !policy.is_desktop_or_dock(*id)
                    && (workspace == active || !policy.is_sticky(*id))
            });
            let frames = shown.filter_map(|id| policy.frame_rect(id));
            let on_output = frames.filter(|frame| frame.overlaps(geometry));
            on_output
                .map(|frame| Rectangle::new(frame.loc - geometry.loc, frame.size))
                .collect()
        };
        let count = policy.workspace_count();
        Drawn {
            layout: self.workspace_popup_layout(output),
            selected: active,
            names: (0..count)
                .map(|workspace| {
                    let name = names.get(workspace).cloned();
                    name.unwrap_or_else(|| default_workspace_name(workspace + 1))
                })
                .collect(),
            windows: (0..count).map(windows).collect(),
            output: geometry.size,
            scale: output.current_scale().fractional_scale().ceil() as i32,
            colors: self.palette.palette().variant(Variant::Default).clone(),
            font: self.settings.interface.font_name.clone(),
            generation: self.frame_context.generation(),
        }
    }

    /// Draws the popup from `drawn`.
    fn draw_workspace_popup(&self, drawn: &Drawn) -> Option<MemoryRenderBuffer> {
        let font = pango::FontDescription::from_string(&drawn.font);
        let context = self.frame_context.text_context();
        let workspaces: Vec<Workspace> = drawn
            .names
            .iter()
            .zip(&drawn.windows)
            .map(|(name, windows)| Workspace {
                name: Title::new(context, &font, 1.0, name),
                windows: windows.clone(),
            })
            .collect();
        draw::draw(
            &drawn.layout,
            drawn.selected,
            &workspaces,
            drawn.output,
            &drawn.colors,
            drawn.scale,
        )
        .inspect_err(|err| tracing::warn!("can't draw the workspace popup: {err}"))
        .ok()
    }
}
