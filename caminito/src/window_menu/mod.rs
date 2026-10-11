//! The window menu's side in the compositor (161, [ADR 0026]): Marco's menu of a window's
//! actions, which Alt+Space, a right click on a titlebar, the Frame's menu button and a
//! client's own request open. Policy has its items, which of them are greyed, the
//! selection, and what choosing one does. Here are the keys and the clicks that drive it
//! while it's open, and where it shows. `labels` has its words, `layout` its rectangles,
//! `draw` draws it, and `scene` puts it on the output.
//!
//! [ADR 0026]: ../../../docs/adr/0026-the-window-menu-is-drawn-in-the-scene.md

pub(crate) mod draw;
pub(crate) mod labels;
pub(crate) mod layout;

use caminito_policy::{MenuEntry, MenuItem, MenuKey, MenuSelection, WindowId, WindowMenu};
use caminito_theme::{PaletteColors, Variant, pango};
use smithay::backend::input::{ButtonState, InputTime, Keycode};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::input::keyboard::{FilterResult, Keysym, KeysymHandle};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use self::labels::Label;
use self::layout::Layout;
use crate::output::output_geometry;
use crate::state::{Caminito, SEAT, State};

/// The compositor's side of the open window menu.
pub(crate) struct OpenWindowMenu {
    /// The window it's for, which policy's menu is for too.
    window: WindowId,
    /// Where its top left corner was asked for, in global logical coordinates.
    anchor: Point<i32, Logical>,
    /// The menu on its output, once the refresh pass has laid it out.
    pub(crate) shown: Option<Shown>,
}

/// The window menu as laid out and drawn. It stays on the output it opened on.
pub(crate) struct Shown {
    pub(crate) output: Output,
    pub(crate) texts: Texts,
    pub(crate) main: Pane,
    /// The submenu, while it's open.
    pub(crate) submenu: Option<Pane>,
    /// What it was drawn from, to draw it again only when that changes.
    drawn: Drawn,
}

/// One menu: the window menu itself, or its submenu.
pub(crate) struct Pane {
    /// Its rectangles, from the output's corner.
    pub(crate) layout: Layout,
    /// It as last drawn, unless cairo couldn't draw it.
    pub(crate) chrome: Option<MemoryRenderBuffer>,
}

/// The words of an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Text {
    pub(crate) label: Label,
    /// Its keybinding, as a menu shows one: `Alt+F9`.
    pub(crate) accelerator: Option<String>,
}

/// The words of the open menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Texts {
    /// Each entry's, in order. A separator has none.
    pub(crate) top: Vec<Option<Text>>,
    /// Each of the submenu's items'.
    pub(crate) sub: Vec<Text>,
}

/// Everything the menu is laid out and drawn from.
#[derive(PartialEq)]
struct Drawn {
    menu: WindowMenu,
    texts: Texts,
    anchor: Point<i32, Logical>,
    output: Rectangle<i32, Logical>,
    scale: i32,
    colors: PaletteColors,
    font: String,
    /// The frames' generation, which counts the changes to the font rendering.
    generation: u64,
}

/// What of the window menu the pointer is on.
enum Hit {
    /// A row.
    Row(MenuSelection),
    /// The menu, between its rows and its edge.
    Edge,
}

impl Caminito {
    /// The window menu, once it's laid out.
    pub(crate) fn shown_window_menu(&self) -> Option<&Shown> {
        self.window_menu.as_ref()?.shown.as_ref()
    }

    /// Brings the window menu up to date with policy's, in the refresh pass: lets go of it
    /// when policy has closed its own, takes up one a binding opened, under its window's
    /// top left corner, as Marco's `handle_activate_window_menu` puts it
    /// (keybindings.c:2969-2991), and lays it out and draws it again if anything it's
    /// drawn from changed. The session lock closes it.
    pub(crate) fn refresh_window_menu(&mut self) {
        if self.lock.is_locked() {
            self.policy.close_window_menu();
        }
        let Some(menu) = self.policy.window_menu().cloned() else {
            if self.window_menu.take().is_some() {
                self.queue_redraw_all();
            }
            return;
        };
        let open = self
            .window_menu
            .take()
            .filter(|open| open.window == menu.window);
        let anchor = match &open {
            Some(open) => open.anchor,
            None => match self.policy.geometry(menu.window) {
                Some(geometry) => geometry.loc,
                None => {
                    self.policy.close_window_menu();
                    self.queue_redraw_all();
                    return;
                }
            },
        };
        let shown = open.and_then(|open| open.shown);
        // The output it's on, or the one it was asked for on.
        let output = shown
            .as_ref()
            .map(|shown| shown.output.clone())
            .filter(|output| self.outputs.contains(output))
            .or_else(|| {
                let has_anchor = |output: &&Output| output_geometry(output).contains(anchor);
                self.outputs.iter().find(has_anchor).cloned()
            })
            .or_else(|| self.popup_output());
        let Some(output) = output else {
            self.policy.close_window_menu();
            self.queue_redraw_all();
            return;
        };
        let drawn = Drawn {
            texts: self.window_menu_texts(&menu),
            menu,
            anchor,
            output: output_geometry(&output),
            scale: output.current_scale().fractional_scale().ceil() as i32,
            colors: self.palette.palette().variant(Variant::Default).clone(),
            font: self.settings.interface.font_name.clone(),
            generation: self.frame_context.generation(),
        };
        let window = drawn.menu.window;
        let shown = match shown {
            Some(shown) if shown.output == output && shown.drawn == drawn => shown,
            _ => {
                let (main, submenu) = self.draw_window_menu(&drawn);
                self.queue_redraw_all();
                Shown {
                    output,
                    texts: drawn.texts.clone(),
                    main,
                    submenu,
                    drawn,
                }
            }
        };
        self.window_menu = Some(OpenWindowMenu {
            window,
            anchor,
            shown: Some(shown),
        });
    }

    /// The words of `menu`'s items, with the keybindings in force.
    fn window_menu_texts(&self, menu: &WindowMenu) -> Texts {
        let names = self.policy.workspace_names();
        let text = |item: &MenuItem| Text {
            label: labels::item(item.op, names),
            accelerator: labels::binding_key(item.op)
                .and_then(|key| self.settings.bindings.label_of(&key)),
        };
        Texts {
            top: menu
                .entries
                .iter()
                .map(|entry| match entry {
                    MenuEntry::Separator => None,
                    MenuEntry::Item(item) => Some(text(item)),
                    MenuEntry::Workspaces(_) => Some(Text {
                        label: labels::submenu(),
                        accelerator: None,
                    }),
                })
                .collect(),
            sub: menu
                .submenu()
                .unwrap_or_default()
                .iter()
                .map(text)
                .collect(),
        }
    }

    /// Lays the menu out and draws it from `drawn`, and its submenu if that's open.
    fn draw_window_menu(&self, drawn: &Drawn) -> (Pane, Option<Pane>) {
        let font = pango::FontDescription::from_string(&drawn.font);
        let context = self.frame_context.text_context();
        let menu = &drawn.menu;
        let item = |text: &Text, item: Option<&MenuItem>| draw::Item {
            label: draw::label(context, &font, &text.label),
            accelerator: text.accelerator.as_deref().map(|accelerator| {
                draw::label(
                    context,
                    &font,
                    &Label::parse(&accelerator.replace('_', "__")),
                )
            }),
            sensitive: item.is_none_or(|item| item.sensitive),
            mark: item.map_or(caminito_policy::MenuMark::None, |item| item.mark),
            submenu: item.is_none(),
        };
        let top: Vec<draw::Row> = menu
            .entries
            .iter()
            .zip(&drawn.texts.top)
            .map(|(entry, text)| match (entry, text) {
                (MenuEntry::Item(entry), Some(text)) => draw::Row::Item(item(text, Some(entry))),
                (MenuEntry::Workspaces(_), Some(text)) => draw::Row::Item(item(text, None)),
                _ => draw::Row::Separator,
            })
            .collect();
        let sub: Vec<draw::Row> = menu
            .submenu()
            .unwrap_or_default()
            .iter()
            .zip(&drawn.texts.sub)
            .map(|(entry, text)| draw::Row::Item(item(text, Some(entry))))
            .collect();
        let text_height = top
            .iter()
            .chain(&sub)
            .find_map(|row| match row {
                draw::Row::Item(item) => Some(item.label.text_height),
                draw::Row::Separator => None,
            })
            .unwrap_or(0);
        let sizes = |rows: &[draw::Row]| -> Vec<layout::Row> {
            rows.iter()
                .map(|row| match row {
                    draw::Row::Separator => layout::Row::Separator,
                    draw::Row::Item(item) => layout::Row::Item {
                        label: item.label.layout.pixel_size().0,
                        accelerator: item
                            .accelerator
                            .as_ref()
                            .map_or(0, |accelerator| accelerator.layout.pixel_size().0),
                    },
                })
                .collect()
        };
        let pane = |layout: Layout, rows: &[draw::Row], selected: Option<usize>| {
            let chrome = draw::draw(&layout, rows, selected, &drawn.colors, drawn.scale)
                .inspect_err(|err| tracing::warn!("can't draw the window menu: {err}"))
                .ok();
            Pane { layout, chrome }
        };
        let output = drawn.output;
        let main = Layout::at(
            &sizes(&top),
            text_height,
            drawn.anchor - output.loc,
            output.size,
        );
        let parent = menu.selected.filter(|_| menu.submenu_open);
        let submenu = parent.and_then(|parent| {
            let parent = *main.rows.get(parent)?;
            let layout = Layout::beside(&sizes(&sub), text_height, main.rect, parent, output.size);
            Some(pane(layout, &sub, menu.submenu_selected))
        });
        (pane(main, &top, menu.selected), submenu)
    }

    /// What of the window menu is at `location`, in global logical coordinates. The
    /// submenu is over the menu.
    fn window_menu_hit(&self, location: Point<f64, Logical>) -> Option<Hit> {
        let shown = self.shown_window_menu()?;
        let point = location - output_geometry(&shown.output).loc.to_f64();
        let hit = |pane: &Pane, row: fn(usize) -> MenuSelection| {
            pane.layout.contains(point).then(|| {
                let at = pane.layout.row_at(point);
                at.map_or(Hit::Edge, |index| Hit::Row(row(index)))
            })
        };
        shown
            .submenu
            .as_ref()
            .and_then(|submenu| hit(submenu, MenuSelection::Sub))
            .or_else(|| hit(&shown.main, MenuSelection::Top))
    }

    /// The pointer moved to `location` while the window menu may be open: the row it's
    /// over is selected, and the submenu opens from its parent.
    pub(crate) fn window_menu_motion(&mut self, location: Point<f64, Logical>) {
        if let Some(Hit::Row(at)) = self.window_menu_hit(location) {
            self.policy.window_menu_select(at);
            self.refresh_window_menu();
        }
    }

    /// The entry, or the submenu's item while that's open, whose mnemonic `typed` is: the
    /// first that isn't greyed, since two labels of a translation can share a letter.
    fn window_menu_mnemonic(&self, typed: char) -> Option<MenuSelection> {
        let menu = self.policy.window_menu()?;
        let texts = &self.shown_window_menu()?.texts;
        if menu.submenu_open {
            let items = menu.submenu().unwrap_or_default();
            let chosen = |(item, text): &(&MenuItem, &Text)| {
                item.sensitive && text.label.is_chosen_by(typed)
            };
            let at = items.iter().zip(&texts.sub).position(|pair| chosen(&pair));
            at.map(MenuSelection::Sub)
        } else {
            let chosen = |(entry, text): &(&MenuEntry, &Option<Text>)| {
                let greyed = matches!(entry, MenuEntry::Item(item) if !item.sensitive);
                let text = text.as_ref();
                !greyed && text.is_some_and(|text| text.label.is_chosen_by(typed))
            };
            let at = menu
                .entries
                .iter()
                .zip(&texts.top)
                .position(|pair| chosen(&pair));
            at.map(MenuSelection::Top)
        }
    }

    /// A key went down while the window menu may be open. Returns what the keyboard path
    /// does with it, or `None` if the menu is closed and the key is the other stages'.
    /// Every key but a modifier is the menu's and reaches no client, as a GTK menu's grab
    /// has it: the arrows, Home and End move in it, Return and space choose, Escape closes
    /// it, and a letter chooses the item it's the mnemonic of.
    pub(crate) fn window_menu_key_pressed<T>(
        &mut self,
        keycode: Keycode,
        keysym: &KeysymHandle<'_>,
    ) -> Option<FilterResult<Option<T>>> {
        // The menu as policy has it now, with its words.
        self.refresh_window_menu();
        self.policy.window_menu()?;
        if keysym.modified_sym().is_modifier_key() {
            return Some(FilterResult::Forward);
        }
        self.suppressed_keys.insert(keycode);
        let now = self.now();
        // Keys are named as the first layout does, like the bindings.
        let key = match keysym.raw_latin_sym_or_raw_current_sym() {
            Some(Keysym::Up | Keysym::KP_Up) => Some(MenuKey::Up),
            Some(Keysym::Down | Keysym::KP_Down) => Some(MenuKey::Down),
            Some(Keysym::Home | Keysym::KP_Home) => Some(MenuKey::Home),
            Some(Keysym::End | Keysym::KP_End) => Some(MenuKey::End),
            Some(Keysym::Left | Keysym::KP_Left) => Some(MenuKey::Left),
            Some(Keysym::Right | Keysym::KP_Right) => Some(MenuKey::Right),
            Some(Keysym::Return | Keysym::KP_Enter | Keysym::space | Keysym::KP_Space) => {
                Some(MenuKey::Activate)
            }
            Some(Keysym::Escape) => Some(MenuKey::Escape),
            _ => None,
        };
        match key {
            Some(key) => self.policy.window_menu_key(key, now),
            None => {
                // By the letter the press types, or the first layout's for the key.
                let typed = keysym.modified_sym().key_char();
                let first = keysym
                    .raw_latin_sym_or_raw_current_sym()
                    .and_then(|sym| sym.key_char());
                let chosen = [typed, first]
                    .into_iter()
                    .flatten()
                    .find_map(|typed| self.window_menu_mnemonic(typed));
                if let Some(at) = chosen {
                    self.policy.window_menu_activate(at, now);
                }
            }
        }
        self.refresh_window_menu();
        Some(FilterResult::Intercept(None))
    }
}

impl State {
    /// Opens `window`'s menu with its top left corner at `at`, in global logical
    /// coordinates, or as near as its output has room for. A client's open menu is
    /// dismissed first. Nothing opens while the session is locked.
    pub(crate) fn open_window_menu(
        &mut self,
        window: WindowId,
        at: Point<i32, Logical>,
        time: InputTime,
    ) {
        if self.caminito.lock.is_locked() {
            return;
        }
        self.dismiss_menus(time);
        let caminito = &mut self.caminito;
        if caminito.policy.open_window_menu(SEAT, window) {
            caminito.window_menu = Some(OpenWindowMenu {
                window,
                anchor: at,
                shown: None,
            });
        }
        caminito.refresh_window_menu();
    }

    /// A pointer button's press with the pointer at `location`. Returns whether it was the
    /// window menu's, and so no client's: while the menu is open, a press on an item
    /// chooses it, and a press outside the menu closes it. The release of such a press is
    /// swallowed with the Window switcher's.
    pub(crate) fn window_menu_button(
        &mut self,
        button: u32,
        state: ButtonState,
        location: Point<f64, Logical>,
    ) -> bool {
        let caminito = &mut self.caminito;
        if state == ButtonState::Released {
            return false;
        }
        caminito.refresh_window_menu();
        if caminito.policy.window_menu().is_none() {
            return false;
        }
        caminito.suppressed_buttons.insert(button);
        let now = caminito.now();
        match caminito.window_menu_hit(location) {
            Some(Hit::Row(at)) => {
                // The row first, as the pointer's motion to it would have.
                caminito.policy.window_menu_select(at);
                caminito.policy.window_menu_activate(at, now);
            }
            Some(Hit::Edge) => {}
            None => caminito.policy.close_window_menu(),
        }
        caminito.refresh_window_menu();
        true
    }
}
