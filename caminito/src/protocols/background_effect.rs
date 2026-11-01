//! The global of `ext_background_effect_manager_v1` (CMN-238). Smithay serves the protocol's
//! objects and keeps a surface's blur region, but it doesn't keep the managers, and the protocol
//! has their `capabilities` sent again each time they change. So this module offers the global
//! itself and keeps what's bound, and hands each manager on to Smithay's requests.

use smithay::reexports::wayland_protocols::ext::background_effect::v1::server::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::wayland::background_effect::{Capability, ExtBackgroundEffectHandler};
use smithay::wayland::{GlobalData, GlobalDispatch2};

/// The version Caminito offers.
const VERSION: u32 = 1;

/// What the compositor does for the global.
pub trait BackgroundEffectHandler: ExtBackgroundEffectHandler {
    fn background_effect_state(&mut self) -> &mut BackgroundEffectState;
}

/// The managers clients have bound.
#[derive(Debug, Default)]
pub struct BackgroundEffectState {
    managers: Vec<ExtBackgroundEffectManagerV1>,
}

/// The global's own data, which sets it apart from Smithay's global of this interface.
pub struct BackgroundEffectGlobal;

impl BackgroundEffectState {
    pub fn new<D>(dh: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<ExtBackgroundEffectManagerV1, BackgroundEffectGlobal> + 'static,
    {
        dh.create_global::<D, ExtBackgroundEffectManagerV1, _>(VERSION, BackgroundEffectGlobal);
        Self::default()
    }

    /// Tells every client the compositor's new `capabilities`.
    pub fn capabilities_changed(&mut self, capabilities: Capability) {
        self.managers.retain(Resource::is_alive);
        for manager in &self.managers {
            manager.capabilities(capabilities);
        }
    }
}

impl<D> GlobalDispatch2<ExtBackgroundEffectManagerV1, D> for BackgroundEffectGlobal
where
    D: BackgroundEffectHandler + Dispatch<ExtBackgroundEffectManagerV1, GlobalData>,
{
    fn bind(
        &self,
        state: &mut D,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ExtBackgroundEffectManagerV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        let manager = data_init.init(resource, GlobalData);
        manager.capabilities(state.capabilities());
        let managers = &mut state.background_effect_state().managers;
        managers.retain(Resource::is_alive);
        managers.push(manager);
    }
}
