//! A `zwlr_foreign_toplevel_management_unstable_v1` client, as a panel's window list is one
//! (65): it records the toplevels the compositor shows and every event it sends for them,
//! and asks for them to be activated, closed, minimised, maximised and made fullscreen. It
//! lists the toplevels of `ext_foreign_toplevel_list_v1` too.
//!
//! [`xfce_foreign_toplevel`](crate::xfce_foreign_toplevel) adds the workspace each toplevel
//! is on (75).

use wayland_client::{Connection, Dispatch, QueueHandle, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    self, ZwlrForeignToplevelHandleV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::{
    self, ZwlrForeignToplevelManagerV1,
};

pub use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::State as HandleState;

use crate::layer_shell::LayerId;
use crate::xfce_foreign_toplevel::raw::xfce_foreign_toplevel_handle_v1::XfceForeignToplevelHandleV1;
use crate::xfce_foreign_toplevel::raw::xfce_foreign_toplevel_manager_private_v1::XfceForeignToplevelManagerPrivateV1;
use crate::{Client, State, WindowId, decode_enums};

/// What foreign toplevel management, and the foreign toplevel list, have told the client.
#[derive(Default)]
pub struct ForeignToplevels {
    manager: Option<ZwlrForeignToplevelManagerV1>,
    /// Xfce's manager, once bound: every toplevel announced after that is asked for its
    /// Xfce handle, as libxfce4windowing asks.
    pub(crate) xfce_manager: Option<XfceForeignToplevelManagerPrivateV1>,
    /// Every toplevel the compositor has announced, in the order it did. A closed one
    /// stays, so a toplevel's index names it for as long as the client lives.
    pub toplevels: Vec<Toplevel>,
    /// Every event since the client last took them, oldest first, as `toplevel <index>:
    /// <event>`.
    events: Vec<String>,
    /// Whether a closed toplevel's handle is kept, to make requests of, rather than
    /// destroyed as the protocol asks.
    pub keep_closed_handles: bool,
    list: Option<ExtForeignToplevelListV1>,
    /// Every toplevel of the foreign toplevel list, in the order the compositor announced
    /// them. A closed one stays.
    pub listed: Vec<ListedToplevel>,
}

/// A toplevel, as its last events left it.
pub struct Toplevel {
    pub(crate) proxy: ZwlrForeignToplevelHandleV1,
    /// Its Xfce handle, if the client asked for one.
    pub(crate) xfce: Option<XfceForeignToplevelHandleV1>,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub states: Vec<HandleState>,
    /// The index of the workspace its Xfce handle last said it's on.
    pub workspace: Option<usize>,
    /// The names of the outputs it's on, in the order it entered them.
    pub outputs: Vec<String>,
    /// The index of its parent.
    pub parent: Option<usize>,
    pub closed: bool,
}

/// A toplevel of `ext_foreign_toplevel_list_v1`.
pub struct ListedToplevel {
    pub(crate) proxy: ExtForeignToplevelHandleV1,
    pub identifier: Option<String>,
    pub title: Option<String>,
    pub app_id: Option<String>,
    /// How many `done` events it has had.
    pub updates: usize,
    pub closed: bool,
}

impl ForeignToplevels {
    /// The index of the open toplevel titled `title`.
    pub fn titled(&self, title: &str) -> usize {
        self.toplevels
            .iter()
            .position(|toplevel| !toplevel.closed && toplevel.title.as_deref() == Some(title))
            .unwrap_or_else(|| panic!("there's a toplevel titled {title}"))
    }

    /// Records an event of the toplevel at `index`.
    pub(crate) fn push_event(&mut self, index: usize, text: &str) {
        self.events.push(format!("toplevel {index}: {text}"));
    }
}

impl Client {
    /// Binds foreign toplevel management. Roundtrip before reading the toplevels.
    pub fn bind_foreign_toplevels(&mut self) {
        let manager = crate::bind(&self.registry, &self.state.globals, &self.qh, 3);
        self.state.foreign_toplevels.manager = Some(manager);
    }

    /// Binds the foreign toplevel list. Roundtrip before reading its toplevels.
    pub fn bind_foreign_toplevel_list(&mut self) {
        let list = crate::bind(&self.registry, &self.state.globals, &self.qh, 1);
        self.state.foreign_toplevels.list = Some(list);
    }

    /// Asks for no more toplevels. The ones the client has stay.
    pub fn stop_foreign_toplevels(&mut self) {
        if let Some(manager) = &self.state.foreign_toplevels.manager {
            manager.stop();
        }
    }

    /// Keeps the handles of toplevels that close from now on, so the client can make
    /// requests of them, which a compositor ignores.
    pub fn keep_closed_toplevel_handles(&mut self) {
        self.state.foreign_toplevels.keep_closed_handles = true;
    }

    /// What foreign toplevel management has told the client.
    pub fn foreign_toplevels(&self) -> &ForeignToplevels {
        &self.state.foreign_toplevels
    }

    /// The events since they were last taken, one per line.
    pub fn take_toplevel_events(&mut self) -> String {
        std::mem::take(&mut self.state.foreign_toplevels.events).join("\n")
    }

    fn toplevel_proxy(&self, index: usize) -> &ZwlrForeignToplevelHandleV1 {
        &self.state.foreign_toplevels.toplevels[index].proxy
    }

    /// Asks for a toplevel to be activated, as a click on its button in a window list does.
    pub fn activate_toplevel(&mut self, index: usize) {
        let seat = self.seat();
        self.toplevel_proxy(index).activate(&seat);
    }

    /// Asks for a toplevel to be closed.
    pub fn close_toplevel(&self, index: usize) {
        self.toplevel_proxy(index).close();
    }

    /// Asks for a toplevel to be maximised, or not.
    pub fn set_toplevel_maximized(&self, index: usize, maximized: bool) {
        let toplevel = self.toplevel_proxy(index);
        if maximized {
            toplevel.set_maximized();
        } else {
            toplevel.unset_maximized();
        }
    }

    /// Asks for a toplevel to be minimised, or not.
    pub fn set_toplevel_minimized(&self, index: usize, minimized: bool) {
        let toplevel = self.toplevel_proxy(index);
        if minimized {
            toplevel.set_minimized();
        } else {
            toplevel.unset_minimized();
        }
    }

    /// Asks for a toplevel to be fullscreen, on the bound output at `output` if one is
    /// given.
    pub fn set_toplevel_fullscreen(&self, index: usize, output: Option<usize>) {
        let output = output.map(|output| &self.state.outputs[output].proxy);
        self.toplevel_proxy(index).set_fullscreen(output);
    }

    /// Asks for a toplevel to leave fullscreen.
    pub fn unset_toplevel_fullscreen(&self, index: usize) {
        self.toplevel_proxy(index).unset_fullscreen();
    }

    /// Says where a toplevel's button is: `rectangle`, as x, y, width and height, on the
    /// surface of the client's window `on`.
    pub fn set_toplevel_rectangle(
        &self,
        index: usize,
        on: WindowId,
        rectangle: (i32, i32, i32, i32),
    ) {
        let (x, y, width, height) = rectangle;
        self.toplevel_proxy(index)
            .set_rectangle(&self.window(on).surface, x, y, width, height);
    }

    /// Says where a toplevel's button is on the client's layer surface `on`, as a panel
    /// whose window list is on its bar does.
    pub fn set_toplevel_rectangle_on_layer(
        &self,
        index: usize,
        on: LayerId,
        rectangle: (i32, i32, i32, i32),
    ) {
        let (x, y, width, height) = rectangle;
        self.toplevel_proxy(index).set_rectangle(
            &self.layer_surface(on).surface,
            x,
            y,
            width,
            height,
        );
    }
}

/// The open toplevels as one line each: the title, the app id, the states, the outputs and
/// the parent's title.
pub fn format_toplevels(toplevels: &[Toplevel]) -> String {
    toplevels
        .iter()
        .filter(|toplevel| !toplevel.closed)
        .map(|toplevel| {
            let text = |text: &Option<String>| text.clone().unwrap_or("(none)".to_owned());
            let parent = toplevel.parent.map_or(String::new(), |parent| {
                format!(", child of {}", text(&toplevels[parent].title))
            });
            format!(
                "{} ({}): {:?} on {:?}{parent}",
                text(&toplevel.title),
                text(&toplevel.app_id),
                toplevel.states,
                toplevel.outputs,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let foreign = &mut state.foreign_toplevels;
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                foreign
                    .events
                    .push(format!("toplevel {}: new", foreign.toplevels.len()));
                let xfce = foreign.xfce_manager.as_ref();
                let xfce =
                    xfce.map(|xfce| xfce.get_xfce_foreign_toplevel_handle(&toplevel, qh, ()));
                foreign.toplevels.push(Toplevel {
                    proxy: toplevel,
                    xfce,
                    title: None,
                    app_id: None,
                    states: Vec::new(),
                    workspace: None,
                    outputs: Vec::new(),
                    parent: None,
                    closed: false,
                });
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => foreign.manager = None,
            _ => {}
        }
    }

    event_created_child!(State, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event;

        let index_of = |state: &Self, proxy: &ZwlrForeignToplevelHandleV1| {
            let toplevels = &state.foreign_toplevels.toplevels;
            toplevels.iter().position(|t| t.proxy == *proxy)
        };
        let Some(index) = index_of(state, proxy) else {
            return;
        };
        let text = match event {
            Event::Title { title } => {
                let text = format!("title {title}");
                state.foreign_toplevels.toplevels[index].title = Some(title);
                text
            }
            Event::AppId { app_id } => {
                let text = format!("app_id {app_id}");
                state.foreign_toplevels.toplevels[index].app_id = Some(app_id);
                text
            }
            Event::OutputEnter { output } => {
                let name = state.output_name(&output);
                let text = format!("output_enter {name}");
                state.foreign_toplevels.toplevels[index].outputs.push(name);
                text
            }
            Event::OutputLeave { output } => {
                let name = state.output_name(&output);
                let outputs = &mut state.foreign_toplevels.toplevels[index].outputs;
                outputs.retain(|entered| *entered != name);
                format!("output_leave {name}")
            }
            Event::State { state: states } => {
                let states: Vec<HandleState> = decode_enums(&states);
                let text = format!("state {states:?}");
                state.foreign_toplevels.toplevels[index].states = states;
                text
            }
            Event::Parent { parent } => {
                let parent = parent.and_then(|parent| index_of(state, &parent));
                state.foreign_toplevels.toplevels[index].parent = parent;
                match parent {
                    Some(parent) => format!("parent toplevel {parent}"),
                    None => "parent none".to_owned(),
                }
            }
            Event::Done => "done".to_owned(),
            Event::Closed => {
                state.foreign_toplevels.toplevels[index].closed = true;
                if !state.foreign_toplevels.keep_closed_handles {
                    // The Xfce handle goes with it, as the protocol asks.
                    if let Some(xfce) = state.foreign_toplevels.toplevels[index].xfce.take() {
                        xfce.destroy();
                    }
                    state.foreign_toplevels.toplevels[index].proxy.destroy();
                }
                "closed".to_owned()
            }
            _ => return,
        };
        state.foreign_toplevels.push_event(index, &text);
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.foreign_toplevels.listed.push(ListedToplevel {
                proxy: toplevel,
                identifier: None,
                title: None,
                app_id: None,
                updates: 0,
                closed: false,
            });
        }
    }

    event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_foreign_toplevel_handle_v1::Event;

        let listed = &mut state.foreign_toplevels.listed;
        let Some(toplevel) = listed.iter_mut().find(|t| t.proxy == *proxy) else {
            return;
        };
        match event {
            Event::Identifier { identifier } => toplevel.identifier = Some(identifier),
            Event::Title { title } => toplevel.title = Some(title),
            Event::AppId { app_id } => toplevel.app_id = Some(app_id),
            Event::Done => toplevel.updates += 1,
            Event::Closed => {
                toplevel.closed = true;
                toplevel.proxy.destroy();
            }
            _ => {}
        }
    }
}
