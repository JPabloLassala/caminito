//! What the keyboard and the pointer can focus: a Wayland surface, an XWayland window, or
//! for the keyboard a layer surface (63).
//! Through the window, Smithay's `KeyboardTarget` follows its ICCCM input model and sends
//! `WM_TAKE_FOCUS` (11), and a drag over it speaks XDND rather than `wl_data_device` (37).
//! The shape is anvil's `KeyboardFocusTarget` and `PointerFocusTarget`
//! (`anvil/src/focus.rs:42`, `:59` and `:526`).

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use smithay::backend::input::{InputTime, KeyState};
use smithay::desktop::{LayerSurface, PopupKind};
use smithay::input::Seat;
use smithay::input::dnd::{DndFocus, OfferData, Source};
use smithay::input::keyboard::{KeyboardTarget, KeysymHandle, ModifiersState};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
    GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
    GestureSwipeUpdateEvent, MotionEvent, PointerTarget, RelativeMotionEvent,
};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Point, Serial};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::selection::data_device::WlOfferData;
use smithay::xwayland::X11Surface;
use smithay::xwayland::xwm::XwmOfferData;

use crate::state::State;

/// The keyboard's focus. It's public because the seat names it, and `State` is.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyboardFocus {
    Surface(WlSurface),
    X11(Box<X11Surface>),
    /// A layer surface, which has the keyboard while policy's focused window keeps the
    /// focus (63).
    Layer(LayerSurface),
}

impl From<WlSurface> for KeyboardFocus {
    fn from(surface: WlSurface) -> Self {
        KeyboardFocus::Surface(surface)
    }
}

impl From<PopupKind> for KeyboardFocus {
    fn from(popup: PopupKind) -> Self {
        KeyboardFocus::Surface(popup.wl_surface().clone())
    }
}

/// Popup grabs hand the keyboard's focus to the pointer. Only xdg popups are grabbed, so
/// the focus is always their Wayland surface.
impl From<KeyboardFocus> for WlSurface {
    fn from(focus: KeyboardFocus) -> Self {
        focus
            .wl_surface()
            .expect("a popup grab's keyboard focus is an xdg popup's surface")
            .into_owned()
    }
}

impl IsAlive for KeyboardFocus {
    fn alive(&self) -> bool {
        match self {
            KeyboardFocus::Surface(surface) => surface.alive(),
            KeyboardFocus::X11(surface) => surface.alive(),
            KeyboardFocus::Layer(layer) => layer.alive(),
        }
    }
}

impl WaylandFocus for KeyboardFocus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            KeyboardFocus::Surface(surface) => Some(Cow::Borrowed(surface)),
            KeyboardFocus::X11(surface) => surface.wl_surface().map(Cow::Owned),
            KeyboardFocus::Layer(layer) => Some(Cow::Borrowed(layer.wl_surface())),
        }
    }
}

impl KeyboardTarget<State> for KeyboardFocus {
    fn enter(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        data.keyboard_entering(self);
        match self {
            KeyboardFocus::Surface(surface) => {
                KeyboardTarget::enter(surface, seat, data, keys, serial)
            }
            KeyboardFocus::X11(surface) => {
                KeyboardTarget::enter(&**surface, seat, data, keys, serial)
            }
            KeyboardFocus::Layer(layer) => {
                if let Some(surface) = keyboard_surface(layer, KeyboardEvent::Enter) {
                    KeyboardTarget::enter(surface, seat, data, keys, serial)
                }
            }
        }
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial) {
        match self {
            KeyboardFocus::Surface(surface) => KeyboardTarget::leave(surface, seat, data, serial),
            KeyboardFocus::X11(surface) => KeyboardTarget::leave(&**surface, seat, data, serial),
            KeyboardFocus::Layer(layer) => {
                if let Some(surface) = keyboard_surface(layer, KeyboardEvent::Leave) {
                    KeyboardTarget::leave(surface, seat, data, serial)
                }
            }
        }
    }

    fn key(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: InputTime,
    ) {
        match self {
            KeyboardFocus::Surface(surface) => {
                KeyboardTarget::key(surface, seat, data, key, state, serial, time)
            }
            KeyboardFocus::X11(surface) => {
                KeyboardTarget::key(&**surface, seat, data, key, state, serial, time)
            }
            KeyboardFocus::Layer(layer) => {
                if let Some(surface) = keyboard_surface(layer, KeyboardEvent::Other) {
                    KeyboardTarget::key(surface, seat, data, key, state, serial, time)
                }
            }
        }
    }

    fn modifiers(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        match self {
            KeyboardFocus::Surface(surface) => {
                KeyboardTarget::modifiers(surface, seat, data, modifiers, serial)
            }
            KeyboardFocus::X11(surface) => {
                KeyboardTarget::modifiers(&**surface, seat, data, modifiers, serial)
            }
            KeyboardFocus::Layer(layer) => {
                if let Some(surface) = keyboard_surface(layer, KeyboardEvent::Other) {
                    KeyboardTarget::modifiers(surface, seat, data, modifiers, serial)
                }
            }
        }
    }
}

/// Whether a layer surface was told the keyboard entered it, kept with the surface.
#[derive(Default)]
struct KeyboardEntered(AtomicBool);

/// A keyboard event for a layer surface, as `keyboard_surface` tells them apart.
enum KeyboardEvent {
    Enter,
    Leave,
    Other,
}

/// The surface the keyboard's events for a layer surface go to, if it heard the keyboard
/// enter. A popup grab hands the keyboard to its root when it ends, on its way back to
/// whatever had it, and a layer surface with no keyboard interactivity, such as the panel
/// under its menu, is to hear nothing of that (63). One that heard the enter always hears
/// the leave, since unmapping resets its interactivity.
fn keyboard_surface(layer: &LayerSurface, event: KeyboardEvent) -> Option<&WlSurface> {
    let entered = &layer.user_data().get_or_insert(KeyboardEntered::default).0;
    let heard = match event {
        KeyboardEvent::Enter => {
            let takes = layer.can_receive_keyboard_focus();
            entered.store(takes, Ordering::Relaxed);
            takes
        }
        KeyboardEvent::Leave => entered.swap(false, Ordering::Relaxed),
        KeyboardEvent::Other => entered.load(Ordering::Relaxed),
    };
    heard.then(|| layer.wl_surface())
}

/// The pointer's focus. An XWayland window is its own target, rather than its surface, so
/// that a drag over it is offered through XDND. It's public because the seat names it.
#[derive(Debug, Clone, PartialEq)]
pub enum PointerFocus {
    Surface(WlSurface),
    X11(Box<X11Surface>),
}

impl From<WlSurface> for PointerFocus {
    fn from(surface: WlSurface) -> Self {
        PointerFocus::Surface(surface)
    }
}

/// Popup grabs give the pointer the keyboard's focus. Only xdg popups are grabbed, so it's
/// always their Wayland surface.
impl From<KeyboardFocus> for PointerFocus {
    fn from(focus: KeyboardFocus) -> Self {
        PointerFocus::Surface(focus.into())
    }
}

impl IsAlive for PointerFocus {
    fn alive(&self) -> bool {
        match self {
            PointerFocus::Surface(surface) => surface.alive(),
            PointerFocus::X11(surface) => surface.alive(),
        }
    }
}

impl WaylandFocus for PointerFocus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            PointerFocus::Surface(surface) => Some(Cow::Borrowed(surface)),
            PointerFocus::X11(surface) => surface.wl_surface().map(Cow::Owned),
        }
    }
}

impl PointerFocus {
    fn target(&self) -> &dyn PointerTarget<State> {
        match self {
            PointerFocus::Surface(surface) => surface,
            PointerFocus::X11(surface) => &**surface,
        }
    }
}

impl PointerTarget<State> for PointerFocus {
    fn enter(&self, seat: &Seat<State>, data: &mut State, event: &MotionEvent) {
        self.target().enter(seat, data, event)
    }

    fn motion(&self, seat: &Seat<State>, data: &mut State, event: &MotionEvent) {
        self.target().motion(seat, data, event)
    }

    fn relative_motion(&self, seat: &Seat<State>, data: &mut State, event: &RelativeMotionEvent) {
        self.target().relative_motion(seat, data, event)
    }

    fn button(&self, seat: &Seat<State>, data: &mut State, event: &ButtonEvent) {
        self.target().button(seat, data, event)
    }

    fn axis(&self, seat: &Seat<State>, data: &mut State, frame: AxisFrame) {
        self.target().axis(seat, data, frame)
    }

    fn frame(&self, seat: &Seat<State>, data: &mut State) {
        self.target().frame(seat, data)
    }

    fn gesture_swipe_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeBeginEvent,
    ) {
        self.target().gesture_swipe_begin(seat, data, event)
    }

    fn gesture_swipe_update(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeUpdateEvent,
    ) {
        self.target().gesture_swipe_update(seat, data, event)
    }

    fn gesture_swipe_end(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeEndEvent,
    ) {
        self.target().gesture_swipe_end(seat, data, event)
    }

    fn gesture_pinch_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchBeginEvent,
    ) {
        self.target().gesture_pinch_begin(seat, data, event)
    }

    fn gesture_pinch_update(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchUpdateEvent,
    ) {
        self.target().gesture_pinch_update(seat, data, event)
    }

    fn gesture_pinch_end(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchEndEvent,
    ) {
        self.target().gesture_pinch_end(seat, data, event)
    }

    fn gesture_hold_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureHoldBeginEvent,
    ) {
        self.target().gesture_hold_begin(seat, data, event)
    }

    fn gesture_hold_end(&self, seat: &Seat<State>, data: &mut State, event: &GestureHoldEndEvent) {
        self.target().gesture_hold_end(seat, data, event)
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial, time: InputTime) {
        self.target().leave(seat, data, serial, time)
    }
}

/// A drag's offer to the pointer's focus, in the focus's protocol.
pub enum Offer<S: Source> {
    Wayland(WlOfferData<S>),
    X11(XwmOfferData<S>),
}

/// The offer in the protocol a focus asked for, or `Err` if it's in the other one.
type Unwrapped<'a, T> = Result<Option<&'a mut T>, ()>;

impl<S: Source> Offer<S> {
    fn wayland(offer: Option<&mut Self>) -> Unwrapped<'_, WlOfferData<S>> {
        match offer {
            None => Ok(None),
            Some(Offer::Wayland(offer)) => Ok(Some(offer)),
            Some(Offer::X11(_)) => Err(()),
        }
    }

    fn x11(offer: Option<&mut Self>) -> Unwrapped<'_, XwmOfferData<S>> {
        match offer {
            None => Ok(None),
            Some(Offer::X11(offer)) => Ok(Some(offer)),
            Some(Offer::Wayland(_)) => Err(()),
        }
    }
}

impl<S: Source> OfferData for Offer<S> {
    fn disable(&self) {
        match self {
            Offer::Wayland(offer) => offer.disable(),
            Offer::X11(offer) => offer.disable(),
        }
    }

    fn drop(&self) {
        match self {
            Offer::Wayland(offer) => offer.drop(),
            Offer::X11(offer) => offer.drop(),
        }
    }

    fn validated(&self) -> bool {
        match self {
            Offer::Wayland(offer) => offer.validated(),
            Offer::X11(offer) => offer.validated(),
        }
    }
}

/// An offer goes back only to a focus of its own protocol. The grab hands each focus the
/// offer its `enter` made, so the other case never happens.
impl DndFocus<State> for PointerFocus {
    type OfferData<S>
        = Offer<S>
    where
        S: Source;

    fn enter<S: Source>(
        &self,
        data: &mut State,
        dh: &DisplayHandle,
        source: Arc<S>,
        seat: &Seat<State>,
        location: Point<f64, Logical>,
        serial: &Serial,
    ) -> Option<Offer<S>> {
        match self {
            PointerFocus::Surface(surface) => {
                DndFocus::enter(surface, data, dh, source, seat, location, serial)
                    .map(Offer::Wayland)
            }
            PointerFocus::X11(surface) => {
                DndFocus::enter(&**surface, data, dh, source, seat, location, serial)
                    .map(Offer::X11)
            }
        }
    }

    fn motion<S: Source>(
        &self,
        data: &mut State,
        offer: Option<&mut Offer<S>>,
        seat: &Seat<State>,
        location: Point<f64, Logical>,
        time: InputTime,
    ) {
        match self {
            PointerFocus::Surface(surface) => {
                let Ok(offer) = Offer::wayland(offer) else {
                    return;
                };
                DndFocus::motion(surface, data, offer, seat, location, time)
            }
            PointerFocus::X11(surface) => {
                let Ok(offer) = Offer::x11(offer) else {
                    return;
                };
                DndFocus::motion(&**surface, data, offer, seat, location, time)
            }
        }
    }

    fn leave<S: Source>(&self, data: &mut State, offer: Option<&mut Offer<S>>, seat: &Seat<State>) {
        match self {
            PointerFocus::Surface(surface) => {
                if let Ok(offer) = Offer::wayland(offer) {
                    DndFocus::leave(surface, data, offer, seat)
                }
            }
            PointerFocus::X11(surface) => {
                if let Ok(offer) = Offer::x11(offer) {
                    DndFocus::leave(&**surface, data, offer, seat)
                }
            }
        }
    }

    fn drop<S: Source>(&self, data: &mut State, offer: Option<&mut Offer<S>>, seat: &Seat<State>) {
        match self {
            PointerFocus::Surface(surface) => {
                if let Ok(offer) = Offer::wayland(offer) {
                    DndFocus::drop(surface, data, offer, seat)
                }
            }
            PointerFocus::X11(surface) => {
                if let Ok(offer) = Offer::x11(offer) {
                    DndFocus::drop(&**surface, data, offer, seat)
                }
            }
        }
    }
}
