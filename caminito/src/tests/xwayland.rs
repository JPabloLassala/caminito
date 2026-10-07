//! The X11 tier: a real Xwayland under the headless fixture, driven by x11rb (14).

use std::time::Duration;

use caminito_test_client::x11::{Names, PropertyValue};
use insta::assert_snapshot;

use super::fixture::{Fixture, Kind};

#[test]
fn the_root_has_smithays_supporting_wm_check_window() {
    let mut f = Fixture::new();
    let x = f.connect(Kind::X11);

    let text = f.x11_call(x, |x| {
        let mut names = Names::new(x);
        let property = x.property(x.root(), "_NET_SUPPORTING_WM_CHECK")?;
        let root = names.format_property(x, "_NET_SUPPORTING_WM_CHECK", property.as_ref())?;
        let Some(PropertyValue::Cardinals(ids)) = property.map(|p| p.value) else {
            unreachable!("the root has a WINDOW property");
        };
        let check = names.property(x, ids[0], "_NET_SUPPORTING_WM_CHECK")?;
        Ok::<_, x11rb::errors::ReplyError>(format!("root: {root}\nw1: {check}\n"))
    });

    assert_snapshot!(text.unwrap());
}

#[test]
fn xwayland_outlives_its_last_client_while_caminito_is_attached() {
    let mut f = Fixture::new();
    let x = f.connect(Kind::X11);
    f.disconnect(x);

    // `-terminate` has no delay, so an Xwayland that would exit has by then.
    f.dispatch_for(Duration::from_secs(2));
    assert!(f.state.caminito.xwayland_display().is_some());

    let x = f.connect_x11();
    let check = f.x11_call(x, |x| x.property(x.root(), "_NET_SUPPORTING_WM_CHECK"));
    assert!(check.unwrap().is_some());
}
