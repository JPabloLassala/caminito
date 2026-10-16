//! The window menu (161): the five places it opens from, its items as the debug dump lists
//! them, each item chosen with the keyboard and with a click, what closes it, and what's
//! drawn.

use caminito_policy::WindowId as PolicyId;
use caminito_theme::{ButtonFunction, GtkState, Rgba, Variant};
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, BTN_RIGHT, ClientId, ClientWindow, Fixture, KEY_3, KEY_A, KEY_C, KEY_D, KEY_DOWN,
    KEY_END, KEY_ENTER, KEY_ESC, KEY_HOME, KEY_I, KEY_L, KEY_LEFT, KEY_LEFTALT, KEY_M, KEY_N,
    KEY_O, KEY_R, KEY_RIGHT, KEY_S, KEY_SPACE, KEY_T, KEY_U, KEY_UP, KEY_W, KEY_X, Kind,
    WindowSpec, on_both_kinds,
};
use crate::output::output_geometry;
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, MARCO_GENERAL, MARCO_WINDOW_KEYBINDINGS};
use crate::state::SEAT;

/// The debug dump's `window_menu` member: `null` while the window menu is closed.
pub(super) fn menu(f: &Fixture) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["window_menu"].clone()
}

pub(super) fn is_open(f: &Fixture) -> bool {
    !menu(f).is_null()
}

pub(super) fn rect(dump: &serde_json::Value) -> Rectangle<i32, Logical> {
    let field = |name: &str| dump[name].as_i64().unwrap() as i32;
    Rectangle::new(
        (field("x"), field("y")).into(),
        (field("width"), field("height")).into(),
    )
}

pub(super) fn middle(rect: Rectangle<i32, Logical>) -> Point<f64, Logical> {
    (rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))).to_f64()
}

/// One line for each of `items`: `---` for a separator, and for an item `~` if it's greyed,
/// its mark, its label, its keybinding, and `>` if it opens the submenu.
fn lines(items: &serde_json::Value) -> Vec<String> {
    let line = |item: &serde_json::Value| {
        if item["kind"] == "separator" {
            return "---".to_owned();
        }
        let grey = if item["greyed"] == true { "~" } else { "" };
        let mark = match item["mark"].as_str() {
            Some("checked") => "[x] ",
            Some("unchecked") => "[ ] ",
            Some("set") => "(o) ",
            Some("unset") => "( ) ",
            _ => "",
        };
        let label = item["label"].as_str().unwrap();
        let mnemonic = item["mnemonic"].as_str().unwrap_or("-");
        let accelerator = match item["accelerator"].as_str() {
            Some(accelerator) => format!("  {accelerator}"),
            None => String::new(),
        };
        let arrow = if item["kind"] == "submenu" { " >" } else { "" };
        format!("{grey}{mark}{label} ({mnemonic}){accelerator}{arrow}")
    };
    let items = items.as_array().unwrap();
    items.iter().map(line).collect()
}

/// The row of the item labelled `label` in `items`, from the output's corner.
fn row_in(items: &serde_json::Value, label: &str) -> Rectangle<i32, Logical> {
    let items = items.as_array().unwrap();
    let item = items.iter().find(|item| item["label"] == label);
    rect(&item.unwrap_or_else(|| panic!("the menu has no {label}"))["rect"])
}

/// The row of the open menu's item labelled `label`.
pub(super) fn row(f: &Fixture, label: &str) -> Rectangle<i32, Logical> {
    row_in(&menu(f)["items"], label)
}

/// Presses and releases `key`.
pub(super) fn tap(f: &mut Fixture, key: u32) {
    f.key(key, true);
    f.key(key, false);
}

/// Alt+Space, Marco's default for `activate-window-menu`.
pub(super) fn alt_space(f: &mut Fixture) {
    f.key(KEY_LEFTALT, true);
    tap(f, KEY_SPACE);
    f.key(KEY_LEFTALT, false);
}

pub(super) fn has_menu_layer(f: &mut Fixture) -> bool {
    let output = f.state.caminito.outputs[0].clone();
    f.state
        .scene_layers(&output)
        .contains(&SceneLayer::WindowMenu)
}

/// One 1920 × 1080 output, Marco's four workspaces in a row, and a client of `kind` with a
/// window that Caminito draws a Frame for.
fn with_window(kind: Kind) -> (Fixture, ClientId, ClientWindow) {
    let (mut f, id) = Fixture::connected(kind);
    let window = f.map(id, WindowSpec::default());
    (f, id, window)
}

on_both_kinds! {
    fn an_item_bound_to_a_mouse_button_names_the_button(kind: Kind) {
        let bind = |key: &str, value: &str| {
            (MARCO_WINDOW_KEYBINDINGS, key.to_owned(), Value::Str(value.into()))
        };
        let mut f = Fixture::with_settings(vec![
            bind("minimize", "<Super>Button8"),
            bind("maximize", "Button2"),
            bind("close", "<Control><Shift>Button10"),
        ]);
        f.add_output((1920, 1080), 1.0);
        let id = f.connect(kind);
        f.map(id, WindowSpec::default());

        alt_space(&mut f);

        let lines = lines(&menu(&f)["items"]);
        assert_eq!(lines[0], "Minimize (n)  Super+Back Button");
        assert_eq!(lines[1], "Maximize (x)  Middle Button");
        assert_eq!(lines.last().unwrap(), "Close (c)  Shift+Ctrl+Button 10");
    }
}

on_both_kinds! {
    fn alt_space_opens_the_focused_windows_menu_under_its_top_left_corner(kind: Kind) {
        let (mut f, _, window) = with_window(kind);
        assert!(!is_open(&f));
        assert!(!has_menu_layer(&mut f));

        alt_space(&mut f);

        let dump = menu(&f);
        assert_eq!(dump["window"], f.pid(window).0);
        assert_eq!(dump["output"], f.state.caminito.outputs[0].name());
        assert_eq!(rect(&dump["rect"]).loc, f.geometry(window).loc);
        assert!(dump["selected"].is_null());
        assert!(dump["submenu"].is_null());
        assert!(has_menu_layer(&mut f));
        assert_eq!(
            lines(&dump["items"]),
            [
                "Minimize (n)  Alt+F9",
                "Maximize (x)",
                "Move (m)  Alt+F7",
                "Resize (r)  Alt+F8",
                "---",
                "[ ] Always on Top (t)",
                "( ) Always on Visible Workspace (a)",
                "(o) Only on This Workspace (o)",
                "Move to Workspace Right (i)  Shift+Ctrl+Alt+Right",
                "Move to Another Workspace (w) >",
                "---",
                "Close (c)  Alt+F4",
            ]
        );
        // Every row is in the menu, each under the one before.
        let whole = rect(&dump["rect"]);
        let rows: Vec<_> = dump["items"].as_array().unwrap().iter().map(|item| rect(&item["rect"])).collect();
        assert!(rows.iter().all(|row| whole.contains_rect(*row)));
        assert!(rows.windows(2).all(|pair| pair[1].loc.y == pair[0].loc.y + pair[0].size.h));
    }
}

on_both_kinds! {
    fn a_right_click_on_the_titlebar_opens_the_menu_at_the_pointer(kind: Kind) {
        let (mut f, _, window) = with_window(kind);
        let frame = f.visible_frame(window);
        let at = Point::<i32, Logical>::from((frame.loc.x + frame.size.w / 2, frame.loc.y + 8));

        f.pointer_move(f64::from(at.x), f64::from(at.y));
        f.pointer_button(BTN_RIGHT, true);

        assert_eq!(menu(&f)["window"], f.pid(window).0);
        assert_eq!(rect(&menu(&f)["rect"]).loc, at);

        // The release of that press isn't a click in the menu.
        f.pointer_button(BTN_RIGHT, false);
        assert!(is_open(&f));
    }
}

on_both_kinds! {
    fn the_frames_menu_button_opens_the_menu_under_it(kind: Kind) {
        let (mut f, _, window) = with_window(kind);
        let button = f.button_rect(window, ButtonFunction::Menu);
        let at = middle(button);

        f.pointer_move(at.x, at.y);
        f.pointer_button(BTN_LEFT, true);
        assert!(!is_open(&f));
        f.pointer_button(BTN_LEFT, false);

        assert_eq!(menu(&f)["window"], f.pid(window).0);
        let below = button.loc + Point::from((0, button.size.h));
        assert_eq!(rect(&menu(&f)["rect"]).loc, below);
    }
}

// `show_window_menu` is xdg_toplevel's request, and `_GTK_SHOW_WINDOW_MENU` is X's, so each
// of the next two tests maps one kind of window.

#[test]
fn an_xdg_client_asks_for_its_windows_menu_at_a_point_of_its_window() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let geometry = f.geometry(window);
    f.pointer_move(
        f64::from(geometry.loc.x + 30),
        f64::from(geometry.loc.y + 10),
    );
    f.pointer_button(BTN_RIGHT, true);
    f.roundtrip(id);
    let serial = f.client(id).state.last_press_serial.unwrap();

    // A serial that's of no event the client was sent opens nothing.
    f.client(id)
        .show_window_menu(window.xdg(), serial.wrapping_add(1000), 30, 10);
    f.roundtrip(id);
    assert!(!is_open(&f));

    f.client(id).show_window_menu(window.xdg(), serial, 30, 10);
    f.roundtrip(id);

    assert_eq!(menu(&f)["window"], f.pid(window).0);
    assert_eq!(
        rect(&menu(&f)["rect"]).loc,
        geometry.loc + Point::from((30, 10))
    );
    // A window with no Frame has the same items.
    assert_eq!(menu(&f)["items"].as_array().unwrap().len(), 12);
    f.pointer_button(BTN_RIGHT, false);
    assert!(is_open(&f));
}

#[test]
fn an_x_client_asks_for_its_windows_menu_with_gtk_show_window_menu() {
    let (mut f, id) = Fixture::connected(Kind::X11);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let geometry = f.geometry(window);
    let at = geometry.loc + Point::from((30, 10));
    let xid = window.x11();

    // GTK names the device, then the point on the root.
    f.x11_call(id, move |x| {
        x.send_client_message(
            xid,
            "_GTK_SHOW_WINDOW_MENU",
            [2, at.x as u32, at.y as u32, 0, 0],
        )
    })
    .unwrap();
    f.dispatch_until(is_open);

    assert_eq!(menu(&f)["window"], f.pid(window).0);
    assert_eq!(rect(&menu(&f)["rect"]).loc, at);
}

/// What a test of one item does: the item's label, the key of its mnemonic, what's done to
/// the window before the menu opens, and whether the item did its work.
struct Case {
    label: &'static str,
    key: u32,
    before: fn(&mut Fixture, PolicyId),
    done: fn(&mut Fixture, ClientWindow) -> bool,
}

fn nothing(_: &mut Fixture, _: PolicyId) {}

/// Nine workspaces in three rows of three with the middle one active, so that there's a
/// workspace on every side, and a window with a Frame on it.
fn in_the_middle_workspace(kind: Kind) -> (Fixture, ClientWindow) {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(3)),
        (MARCO_GENERAL, "num-workspaces".into(), Value::Int(9)),
    ]);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(kind);
    f.state.caminito.policy.switch_workspace(4);
    let window = f.map(id, WindowSpec::default());
    (f, window)
}

/// Every item of the menu but the submenu's.
fn cases() -> Vec<Case> {
    vec![
        Case {
            label: "Minimize",
            key: KEY_N,
            before: nothing,
            done: |f, w| f.state.caminito.policy.is_minimized(f.pid(w)),
        },
        Case {
            label: "Maximize",
            key: KEY_X,
            before: nothing,
            done: |f, w| f.state.caminito.policy.is_maximized(f.pid(w)),
        },
        Case {
            label: "Unmaximize",
            key: KEY_X,
            before: |f, id| f.state.caminito.policy.set_maximized(id, true),
            done: |f, w| !f.state.caminito.policy.is_maximized(f.pid(w)),
        },
        Case {
            label: "Move",
            key: KEY_M,
            before: nothing,
            done: |f, w| {
                let policy = &f.state.caminito.policy;
                policy.keyboard_grab(SEAT) == Some(f.pid(w)) && policy.grab_edges(SEAT).is_none()
            },
        },
        Case {
            label: "Resize",
            key: KEY_R,
            before: nothing,
            done: |f, w| {
                let policy = &f.state.caminito.policy;
                policy.keyboard_grab(SEAT) == Some(f.pid(w)) && policy.grab_edges(SEAT).is_some()
            },
        },
        Case {
            label: "Move Titlebar Onscreen",
            key: KEY_S,
            before: |f, id| {
                // Above the output, as a pointer's move leaves it.
                let policy = &mut f.state.caminito.policy;
                assert!(policy.begin_move(SEAT, id, (0., 0.).into()));
                policy.grab_motion(SEAT, (0., -2000.).into(), false);
                policy.end_grab(SEAT);
                assert!(!policy.titlebar_is_onscreen(id));
            },
            done: |f, w| f.state.caminito.policy.titlebar_is_onscreen(f.pid(w)),
        },
        Case {
            label: "Always on Top",
            key: KEY_T,
            before: nothing,
            done: |f, w| f.state.caminito.policy.is_above(f.pid(w)),
        },
        Case {
            label: "Always on Visible Workspace",
            key: KEY_A,
            before: nothing,
            done: |f, w| f.state.caminito.policy.is_sticky(f.pid(w)),
        },
        Case {
            label: "Only on This Workspace",
            key: KEY_O,
            before: |f, id| f.state.caminito.policy.set_sticky(id, true),
            done: |f, w| !f.state.caminito.policy.is_sticky(f.pid(w)),
        },
        Case {
            label: "Move to Workspace Left",
            key: KEY_L,
            before: nothing,
            done: |f, w| f.state.caminito.policy.workspace_of(f.pid(w)) == Some(3),
        },
        Case {
            label: "Move to Workspace Right",
            key: KEY_I,
            before: nothing,
            done: |f, w| f.state.caminito.policy.workspace_of(f.pid(w)) == Some(5),
        },
        Case {
            label: "Move to Workspace Up",
            key: KEY_U,
            before: nothing,
            done: |f, w| f.state.caminito.policy.workspace_of(f.pid(w)) == Some(1),
        },
        Case {
            label: "Move to Workspace Down",
            key: KEY_D,
            before: nothing,
            done: |f, w| f.state.caminito.policy.workspace_of(f.pid(w)) == Some(7),
        },
        Case {
            label: "Close",
            key: KEY_C,
            before: nothing,
            done: |f, w| f.close_requested(w),
        },
    ]
}

/// Runs every case: `choose` chooses the case's item in the open menu.
fn each_item(kind: Kind, choose: fn(&mut Fixture, &Case)) {
    for case in cases() {
        // A window of its own, since an item leaves the last one minimised, on another
        // workspace or held by the keyboard.
        let (mut f, window) = in_the_middle_workspace(kind);
        let id = f.pid(window);
        (case.before)(&mut f, id);
        f.dispatch();
        assert!(
            !(case.done)(&mut f, window),
            "{} is still to do",
            case.label
        );
        alt_space(&mut f);
        assert!(is_open(&f), "the menu opens for {}", case.label);

        choose(&mut f, &case);

        assert!(!is_open(&f), "{} closes the menu", case.label);
        f.dispatch_until_mut(|f| (case.done)(f, window));
        // The active workspace stays, whatever the item moved.
        assert_eq!(
            f.state.caminito.policy.active_workspace(),
            4,
            "{}",
            case.label
        );
    }
}

on_both_kinds! {
    fn each_item_is_chosen_by_its_mnemonic(kind: Kind) {
        each_item(kind, |f, case| tap(f, case.key));
    }
}

on_both_kinds! {
    fn each_item_is_chosen_by_a_click(kind: Kind) {
        each_item(kind, |f, case| {
            let at = middle(row(f, case.label));
            f.click(at.x, at.y);
        });
    }
}

on_both_kinds! {
    fn the_arrows_and_return_choose_an_item(kind: Kind) {
        let (mut f, _, window) = with_window(kind);
        alt_space(&mut f);

        tap(&mut f, KEY_DOWN);
        assert_eq!(menu(&f)["selected"], 0);
        tap(&mut f, KEY_DOWN);
        assert_eq!(menu(&f)["selected"], 1);
        tap(&mut f, KEY_END);
        assert_eq!(menu(&f)["selected"], 11);
        tap(&mut f, KEY_HOME);
        tap(&mut f, KEY_UP);
        assert_eq!(menu(&f)["selected"], 11);
        tap(&mut f, KEY_DOWN);
        tap(&mut f, KEY_DOWN);
        tap(&mut f, KEY_ENTER);

        assert!(!is_open(&f));
        assert!(f.state.caminito.policy.is_maximized(f.pid(window)));
    }
}

on_both_kinds! {
    fn the_submenu_lists_the_workspaces_and_moves_the_window_by_keys_and_by_clicks(kind: Kind) {
        let (mut f, _, window) = with_window(kind);
        let id = f.pid(window);

        // Its mnemonic opens it, on its first item that isn't greyed.
        alt_space(&mut f);
        tap(&mut f, KEY_W);
        let submenu = menu(&f)["submenu"].clone();
        assert_eq!(
            lines(&submenu["items"]),
            [
                "~Workspace 1 (1)",
                "Workspace 2 (2)",
                "Workspace 3 (3)",
                "Workspace 4 (4)",
            ]
        );
        assert_eq!(submenu["selected"], 1);
        // To the right of the menu, with its first row beside its parent.
        let parent = row(&f, "Move to Another Workspace");
        let whole = rect(&menu(&f)["rect"]);
        assert_eq!(row_in(&submenu["items"], "Workspace 1").loc.y, parent.loc.y);
        assert!(rect(&submenu["rect"]).loc.x >= whole.loc.x + whole.size.w - 2);

        // Left closes it, Right opens it again, and a workspace's number chooses it.
        tap(&mut f, KEY_LEFT);
        assert!(menu(&f)["submenu"].is_null());
        tap(&mut f, KEY_RIGHT);
        assert_eq!(menu(&f)["submenu"]["selected"], 1);
        tap(&mut f, KEY_3);
        assert!(!is_open(&f));
        assert_eq!(f.state.caminito.policy.workspace_of(id), Some(2));
        assert_eq!(f.state.caminito.policy.active_workspace(), 0);

        // With the pointer: over its parent it opens, and a click on a workspace moves the
        // window there.
        let now = f.state.caminito.now();
        f.state.caminito.policy.move_to_workspace(id, 0);
        f.state.caminito.policy.activate(SEAT, id, now);
        f.dispatch();
        alt_space(&mut f);
        let parent = middle(row(&f, "Move to Another Workspace"));
        f.pointer_move(parent.x, parent.y);
        assert_eq!(menu(&f)["selected"], 9);
        assert!(menu(&f)["submenu"]["selected"].is_null());
        let fourth = middle(row_in(&menu(&f)["submenu"]["items"], "Workspace 4"));
        f.pointer_move(fourth.x, fourth.y);
        assert_eq!(menu(&f)["submenu"]["selected"], 3);
        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);
        assert!(!is_open(&f));
        assert_eq!(f.state.caminito.policy.workspace_of(id), Some(3));
    }
}

on_both_kinds! {
    fn escape_and_a_click_outside_close_the_menu_and_reach_no_client(kind: Kind) {
        let (mut f, id, window) = with_window(kind);
        let other = f.map(id, WindowSpec::default());
        let focused = f.pid(other);
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(focused));

        alt_space(&mut f);
        tap(&mut f, KEY_ESC);
        assert!(!is_open(&f));
        assert!(!has_menu_layer(&mut f));

        // A press outside the menu, on the other window, closes it and focuses nothing.
        alt_space(&mut f);
        let whole = rect(&menu(&f)["rect"]);
        let outside = f.visible_frame(window);
        let at = Point::<i32, Logical>::from((
            outside.loc.x + outside.size.w - 5,
            outside.loc.y + outside.size.h - 20,
        ));
        if !whole.contains(at) {
            f.pointer_move(f64::from(at.x), f64::from(at.y));
        } else {
            f.pointer_move(1900., 1000.);
        }
        f.pointer_button(BTN_LEFT, true);
        assert!(!is_open(&f));
        f.pointer_button(BTN_LEFT, false);
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(focused));

        // A press on the menu's edge, off every row, leaves it open.
        alt_space(&mut f);
        let whole = rect(&menu(&f)["rect"]);
        f.click(f64::from(whole.loc.x + 1), f64::from(whole.loc.y + 1));
        assert!(is_open(&f));
    }
}

on_both_kinds! {
    fn a_greyed_item_cant_be_chosen(kind: Kind) {
        let (mut f, _, window) = with_window(kind);
        let id = f.pid(window);
        f.state.caminito.policy.set_maximized(id, true);
        f.dispatch();

        alt_space(&mut f);

        let items = menu(&f)["items"].clone();
        let greyed: Vec<&str> = items
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["greyed"] == true)
            .map(|item| item["label"].as_str().unwrap())
            .collect();
        assert_eq!(greyed, ["Move", "Resize"]);

        tap(&mut f, KEY_M);
        assert!(is_open(&f));
        let at = middle(row(&f, "Resize"));
        f.click(at.x, at.y);
        assert!(is_open(&f));
        assert_eq!(f.state.caminito.policy.keyboard_grab(SEAT), None);
        // The pointer selects no greyed item, and the arrows go past them.
        assert!(menu(&f)["selected"].is_null());
        tap(&mut f, KEY_DOWN);
        tap(&mut f, KEY_DOWN);
        tap(&mut f, KEY_DOWN);
        assert_eq!(menu(&f)["selected"], 5);
    }
}

// The tests from here map one kind of window: where the menu is and how it's drawn are the
// same whatever the window is.

#[test]
fn the_menu_stays_on_the_output_it_was_asked_for_on() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let second = f.add_output((1280, 720), 1.0);
    let on_second = output_geometry(&second);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let geometry = f.geometry(window);
    f.pointer_move(
        f64::from(geometry.loc.x + 30),
        f64::from(geometry.loc.y + 10),
    );
    f.pointer_button(BTN_RIGHT, true);
    f.roundtrip(id);
    let serial = f.client(id).state.last_press_serial.unwrap();
    f.pointer_button(BTN_RIGHT, false);

    // Asked for off the window, by the second output's bottom right corner.
    let corner = on_second.loc + Point::from((on_second.size.w - 10, on_second.size.h - 10));
    let from_window = corner - geometry.loc;
    f.client(id)
        .show_window_menu(window.xdg(), serial, from_window.x, from_window.y);
    f.roundtrip(id);

    assert_eq!(menu(&f)["output"], second.name());
    let whole = rect(&menu(&f)["rect"]);
    assert_eq!(
        whole.loc + whole.size.to_point(),
        Point::from((on_second.size.w, on_second.size.h))
    );
    let layers = f.state.scene_layers(&second);
    assert!(layers.contains(&SceneLayer::WindowMenu));
    assert!(!has_menu_layer(&mut f));
}

#[test]
fn the_menu_closes_as_the_session_locks_and_none_opens_while_it_is_locked() {
    let (mut f, _, _) = with_window(Kind::Xdg);
    alt_space(&mut f);
    assert!(is_open(&f));

    let locker = f.connect(Kind::Xdg);
    f.client(locker).lock_session();
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "locked");
    assert!(!is_open(&f));

    alt_space(&mut f);
    assert!(!is_open(&f));
}

/// The first output's pixel at a logical point, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.caminito.outputs[0].clone();
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(&output, at.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// How far `pixel` is from `color`, on the channel where it's farthest.
fn distance(pixel: u32, color: Rgba) -> u32 {
    let channel = |shift: u32, value: f64| {
        let pixel = (pixel >> shift) & 0xff;
        pixel.abs_diff((value * 255.).round() as u32)
    };
    channel(16, color.red)
        .max(channel(8, color.green))
        .max(channel(0, color.blue))
}

/// The least distance from `color` of the pixels where `row`'s label is drawn.
fn nearest_in_label(f: &mut Fixture, row: Rectangle<i32, Logical>, color: Rgba) -> u32 {
    let mut nearest = u32::MAX;
    for y in row.loc.y..row.loc.y + row.size.h {
        for x in row.loc.x + 24..row.loc.x + 70 {
            nearest = nearest.min(distance(pixel(f, (x, y).into()), color));
        }
    }
    nearest
}

#[test]
fn the_menu_is_drawn_with_its_items_a_greyed_one_and_the_selected_one() {
    let (mut f, _, window) = with_window(Kind::Xdg);
    let id = f.pid(window);
    f.state.caminito.policy.set_maximized(id, true);
    f.dispatch();
    alt_space(&mut f);
    tap(&mut f, KEY_DOWN);
    let output = f.state.caminito.outputs[0].clone();
    f.draw(&output);

    let colors = f
        .state
        .caminito
        .palette
        .palette()
        .variant(Variant::Default)
        .clone();
    let background = colors.background(GtkState::Normal);
    let text = colors.text(GtkState::Normal);
    let insensitive = colors.text(GtkState::Insensitive);

    // The menu's background, just inside its border, and in a row that isn't selected.
    let whole = rect(&menu(&f)["rect"]);
    assert_eq!(
        distance(pixel(&mut f, whole.loc + Point::from((2, 2))), background),
        0
    );
    let unmaximize = row(&f, "Unmaximize");
    let in_arrow_column =
        |row: Rectangle<i32, Logical>| row.loc + Point::from((row.size.w - 10, row.size.h / 2));
    assert_eq!(
        distance(pixel(&mut f, in_arrow_column(unmaximize)), background),
        0
    );

    // The selected row is filled.
    let minimize = row(&f, "Minimize");
    assert_ne!(
        distance(pixel(&mut f, in_arrow_column(minimize)), background),
        0
    );

    // A label is in the text colour, and a greyed one isn't, though it's drawn. The tests'
    // palette has one text colour for both, so the greyed one is drawn fainter.
    assert!(nearest_in_label(&mut f, unmaximize, text) < 24);
    let greyed = row(&f, "Move");
    assert_eq!(text, insensitive);
    assert!(nearest_in_label(&mut f, greyed, text) > 24);
    assert!(nearest_in_label(&mut f, greyed, background) == 0);
    let drawn = (greyed.loc.x + 24..greyed.loc.x + 70).any(|x| {
        let ys = greyed.loc.y..greyed.loc.y + greyed.size.h;
        ys.into_iter()
            .any(|y| distance(pixel(&mut f, (x, y).into()), background) > 24)
    });
    assert!(drawn);

    // The submenu's arrow is drawn in its parent's arrow column, and a separator's line
    // across its row.
    let parent = row(&f, "Move to Another Workspace");
    assert_ne!(
        distance(pixel(&mut f, in_arrow_column(parent)), background),
        0
    );
    let separator = rect(&menu(&f)["items"][4]["rect"]);
    let on_line = separator.loc + Point::from((separator.size.w / 2, separator.size.h / 2));
    assert_ne!(distance(pixel(&mut f, on_line), background), 0);
    assert_eq!(
        distance(pixel(&mut f, on_line - Point::from((0, 2))), background),
        0
    );

    // A set radio button has its dot, and an unset one has none.
    let middle_of_mark = |row: Rectangle<i32, Logical>| row.loc + Point::from((12, row.size.h / 2));
    let set = middle_of_mark(row(&f, "Only on This Workspace"));
    let unset = middle_of_mark(row(&f, "Always on Visible Workspace"));
    assert_ne!(distance(pixel(&mut f, set), background), 0);
    assert_eq!(distance(pixel(&mut f, unset), background), 0);
}
