//! The window: Plasma's page, as the prototype settled it (84), in the Displays tab. The
//! arrangement of the displays on top, a row with one entry per display, the selected
//! display's settings below, and Identify, Reset and Apply at the bottom. Nothing outside the
//! window changes until Apply, and what Apply changes is kept or reverted within the
//! countdown. The Night Light tab beside it (142) is `night_light_tab`'s.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use caminito_displays::outputs::Outcome;
use caminito_displays::page::{Layout, Page, Scale, rate_label};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::arrangement::{self, View};
use crate::compositor::{Compositor, Event};
use crate::identify;
use crate::night_light_tab::NightLightTab;

/// How long a new configuration is shown before it's reverted unless kept, in seconds
/// (the prototype's verdict).
const COUNTDOWN: u32 = 15;

/// The arrangement's height, as the prototype has it.
const ARRANGEMENT_HEIGHT: i32 = 300;

/// What the page says beside Primary, by what the selected display is to it.
const PRIMARY_NOTES: [(&str, &str); 3] = [
    (
        "here",
        "The panel and the desktop icons are here.\nChoose another display to move them.",
    ),
    (
        "elsewhere",
        "Puts the panel, the desktop icons and new windows here.",
    ),
    ("none", "Only Caminito has a primary display."),
];

const CSS: &str = "
.caminito-displays-scrim { background-color: rgba(0, 0, 0, 0.45); }
.caminito-displays-dialog {
  border: 1px solid alpha(currentColor, 0.3);
  border-radius: 8px;
  padding: 20px;
}
";

/// The window's tabs, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Displays,
    NightLight,
}

impl Tab {
    /// Its name, as the window's `show-tab` action takes it.
    fn name(self) -> &'static str {
        match self {
            Tab::Displays => "displays",
            Tab::NightLight => "night-light",
        }
    }

    fn from_name(name: &str) -> Option<Tab> {
        [Tab::Displays, Tab::NightLight]
            .into_iter()
            .find(|tab| tab.name() == name)
    }

    /// Its page in the notebook.
    fn page(self) -> u32 {
        match self {
            Tab::Displays => 0,
            Tab::NightLight => 1,
        }
    }
}

/// Switches the open window to `tab`. A window that only says why it can't show the
/// displays stays as it is.
pub(crate) fn show(window: &gtk::Window, tab: Tab) {
    let _ = window.activate_action("win.show-tab", Some(&tab.name().to_variant()));
}

/// What there is to go back to if a configuration isn't kept.
struct Before {
    /// The layout Caminito showed.
    layout: Layout,
    /// The primary output Caminito had, if this window is choosing another: it's chosen
    /// again on the way back.
    primary: Option<String>,
}

/// What the window is doing about a configuration.
enum Phase {
    /// Nothing: the page is the user's to change.
    Editing,
    /// A configuration is sent, and Caminito hasn't answered. `primary` is the primary
    /// display the user chose with it, to choose once Caminito shows the configuration: the
    /// display may be one the configuration turns on.
    Applying {
        before: Before,
        primary: Option<String>,
    },
    /// Caminito shows the configuration, and the user hasn't kept it yet.
    Confirming {
        before: Before,
        left: u32,
        tick: glib::SourceId,
    },
    /// The configuration that goes back is sent, and Caminito hasn't answered. `primary`
    /// is the primary output to choose again once it has: it may be one the
    /// configuration turns back on.
    Reverting { primary: Option<String> },
    /// The compositor is gone.
    Lost,
}

/// A press on the arrangement that may become a drag.
#[derive(Clone, Copy)]
struct Press {
    /// The view the display was picked up in, which stays while it's in the air, so the
    /// arrangement doesn't zoom under the pointer.
    view: View,
    moved: bool,
}

/// Selects the display at an index of the page.
type Select = Rc<dyn Fn(usize)>;

struct Ui {
    compositor: Compositor,
    page: RefCell<Page>,
    phase: RefCell<Phase>,
    /// Set while the widgets are given the page's values, so that their handlers don't
    /// take them for the user's.
    rendering: Cell<bool>,
    press: Cell<Option<Press>>,
    /// The labels of Identify are up until this fires.
    identifying: RefCell<Option<glib::SourceId>>,

    window: gtk::ApplicationWindow,
    notebook: gtk::Notebook,
    /// Held for as long as the window is.
    _night_light: Rc<NightLightTab>,
    content: gtk::Box,
    canvas: gtk::DrawingArea,
    tabs: gtk::Box,
    tab_buttons: RefCell<Vec<(String, gtk::ToggleButton)>>,
    /// What an entry of the row does when it's pressed: it selects its display.
    select: RefCell<Option<Select>>,
    form: gtk::Grid,
    title: gtk::Label,
    connector: gtk::Label,
    enabled: gtk::CheckButton,
    enabled_note: gtk::Label,
    primary: gtk::CheckButton,
    primary_note: gtk::Stack,
    resolution: gtk::DropDown,
    rate: gtk::DropDown,
    scale: gtk::Scale,
    scale_label: gtk::Label,
    scale_note: gtk::Label,
    /// The rows that mean nothing for a display that's off.
    rows_of_an_enabled_display: Vec<gtk::Widget>,
    message: gtk::Label,
    identify: gtk::Button,
    reset: gtk::Button,
    apply: gtk::Button,
    scrim: gtk::Box,
    countdown: gtk::Label,
    revert: gtk::Button,
    keep: gtk::Button,
}

/// Opens the window on `tab`, on Caminito or, for `demo`, on made-up monitors and a
/// made-up Night Light.
pub(crate) fn open(application: &gtk::Application, demo: bool, tab: Tab) {
    let connected = if demo {
        Ok(Compositor::demo())
    } else {
        Compositor::connect()
    };
    let compositor = match connected {
        Ok(compositor) => compositor,
        Err(err) => return sorry(application, &err.to_string()),
    };
    if let Some(display) = gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let night_light = NightLightTab::new(demo, compositor.caminito());
    let ui = Rc::new(Ui::build(application, compositor, night_light));
    ui.connect();
    ui.render();
    ui.notebook.set_current_page(Some(tab.page()));
    ui.window.present();
}

/// A window that only says why caminito-displays can't show the displays.
fn sorry(application: &gtk::Application, why: &str) {
    let text = format!("The displays can't be shown here: {why}.");
    let label = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    let window = window(application);
    window.set_child(Some(&label));
    window.present();
}

fn window(application: &gtk::Application) -> gtk::ApplicationWindow {
    gtk::ApplicationWindow::builder()
        .application(application)
        .title("Displays")
        .icon_name("preferences-desktop-display")
        .default_width(780)
        .build()
}

/// A line of quieter text beside a setting.
fn note() -> gtk::Label {
    // One line, or the lines its text has: a label that wraps beside a setting makes the
    // window's height depend on its width.
    let note = gtk::Label::builder().xalign(0.0).build();
    note.add_css_class("dim-label");
    note
}

fn row(children: &[&gtk::Widget]) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    for child in children {
        row.append(*child);
    }
    row
}

/// Gives a list the entries `labels`, unless it has them already. The page is shown
/// again from the list's own handler, with its popover still open, and GTK never returns
/// from a new model given to it there (85). Choosing an entry leaves the entries of its
/// own list as they were, which the page's tests hold it to, so the list that is replaced
/// is never the one being chosen from.
fn fill(dropdown: &gtk::DropDown, labels: &[String]) {
    let entries = dropdown.model().and_downcast::<gtk::StringList>();
    let same = entries.is_some_and(|entries| {
        entries.n_items() as usize == labels.len()
            && (0..entries.n_items())
                .zip(labels)
                .all(|(at, label)| entries.string(at).is_some_and(|entry| entry == *label))
    });
    if !same {
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        dropdown.set_model(Some(&gtk::StringList::new(&labels)));
    }
}

impl Ui {
    fn build(
        application: &gtk::Application,
        compositor: Compositor,
        night_light: Rc<NightLightTab>,
    ) -> Ui {
        let page = Page::new(&compositor.monitors(), compositor.primary().as_deref());

        let canvas = gtk::DrawingArea::builder()
            .content_height(ARRANGEMENT_HEIGHT)
            .hexpand(true)
            .vexpand(true)
            .build();

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        tabs.set_margin_top(12);
        tabs.set_margin_start(18);
        tabs.set_margin_end(18);
        tabs.set_halign(gtk::Align::Center);

        let form = gtk::Grid::builder()
            .row_spacing(12)
            .column_spacing(14)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        let name = |text: &str, at: i32| {
            let label = gtk::Label::builder().label(text).xalign(1.0).build();
            label.add_css_class("dim-label");
            label.set_size_request(150, -1);
            form.attach(&label, 0, at, 1, 1);
            label
        };

        let title = gtk::Label::new(None);
        let connector = note();
        name("Display", 0);
        form.attach(
            &row(&[title.upcast_ref(), connector.upcast_ref()]),
            1,
            0,
            1,
            1,
        );

        let enabled = gtk::CheckButton::with_label("Enabled");
        let enabled_note = note();
        form.attach(
            &row(&[enabled.upcast_ref(), enabled_note.upcast_ref()]),
            1,
            1,
            1,
            1,
        );

        let primary = gtk::CheckButton::with_label("Primary");
        // Every note there is for it, one shown: the row is as tall as the tallest, so
        // the arrangement above doesn't move as another display is selected.
        let primary_note = gtk::Stack::new();
        for (name, text) in PRIMARY_NOTES {
            let note = note();
            note.set_label(text);
            primary_note.add_named(&note, Some(name));
        }
        let primary_row = row(&[primary.upcast_ref(), primary_note.upcast_ref()]);
        form.attach(&primary_row, 1, 2, 1, 1);

        let dropdown = || {
            let dropdown = gtk::DropDown::from_strings(&[]);
            dropdown.set_halign(gtk::Align::Start);
            dropdown.set_size_request(260, -1);
            dropdown
        };
        let resolution = dropdown();
        let resolution_name = name("Resolution", 3);
        form.attach(&resolution, 1, 3, 1, 1);
        let rate = dropdown();
        let rate_name = name("Refresh rate", 4);
        form.attach(&rate, 1, 4, 1, 1);

        let (least, most, step) = (
            f64::from(Scale::MIN_PERCENT),
            f64::from(Scale::MAX_PERCENT),
            f64::from(Scale::STEP_PERCENT),
        );
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, least, most, step);
        scale.set_draw_value(false);
        scale.set_size_request(240, -1);
        let mut mark = least;
        while mark <= most {
            scale.add_mark(mark, gtk::PositionType::Bottom, None);
            mark += step;
        }
        let scale_label = gtk::Label::new(None);
        let scale_note = note();
        let scale_row = row(&[
            scale.upcast_ref(),
            scale_label.upcast_ref(),
            scale_note.upcast_ref(),
        ]);
        let scale_name = name("Scale", 5);
        form.attach(&scale_row, 1, 5, 1, 1);

        let message = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .margin_start(18)
            .margin_end(18)
            .margin_bottom(12)
            .visible(false)
            .build();

        let identify = gtk::Button::with_label("Identify");
        let reset = gtk::Button::with_label("Reset");
        let apply = gtk::Button::with_label("Apply");
        apply.add_css_class("suggested-action");
        let spacer = gtk::Box::builder().hexpand(true).build();
        let actions = row(&[
            identify.upcast_ref(),
            spacer.upcast_ref(),
            reset.upcast_ref(),
            apply.upcast_ref(),
        ]);
        actions.set_spacing(8);
        actions.set_margin_top(12);
        actions.set_margin_bottom(12);
        actions.set_margin_start(18);
        actions.set_margin_end(18);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&canvas);
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        content.append(&tabs);
        content.append(&form);
        content.append(&message);
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        content.append(&actions);

        // "Keep this configuration?", in the window and over the page: a dialog of its
        // own would be a second window to find while the displays are changing.
        let question = gtk::Label::builder().xalign(0.0).build();
        question.set_markup("<b><big>Keep this configuration?</big></b>");
        let countdown = gtk::Label::builder().xalign(0.0).wrap(true).build();
        let revert = gtk::Button::with_label("Revert");
        let keep = gtk::Button::with_label("Keep");
        keep.add_css_class("suggested-action");
        let answers = row(&[revert.upcast_ref(), keep.upcast_ref()]);
        answers.set_spacing(8);
        answers.set_halign(gtk::Align::End);
        answers.set_margin_top(10);
        let dialog = gtk::Box::new(gtk::Orientation::Vertical, 6);
        dialog.add_css_class("background");
        dialog.add_css_class("caminito-displays-dialog");
        dialog.set_halign(gtk::Align::Center);
        dialog.set_valign(gtk::Align::Center);
        dialog.set_size_request(380, -1);
        dialog.append(&question);
        dialog.append(&countdown);
        dialog.append(&answers);
        let scrim = gtk::Box::builder().visible(false).build();
        scrim.add_css_class("caminito-displays-scrim");
        dialog.set_hexpand(true);
        scrim.append(&dialog);

        let notebook = gtk::Notebook::new();
        notebook.append_page(&content, Some(&gtk::Label::new(Some("Displays"))));
        notebook.append_page(
            night_light.widget(),
            Some(&gtk::Label::new(Some("Night Light"))),
        );

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&notebook));
        overlay.add_overlay(&scrim);
        let window = window(application);
        window.set_child(Some(&overlay));

        let rows_of_an_enabled_display = vec![
            primary_row.upcast(),
            resolution_name.upcast(),
            resolution.clone().upcast(),
            rate_name.upcast(),
            rate.clone().upcast(),
            scale_name.upcast(),
            scale_row.upcast(),
        ];
        Ui {
            compositor,
            page: RefCell::new(page),
            phase: RefCell::new(Phase::Editing),
            rendering: Cell::new(false),
            press: Cell::new(None),
            identifying: RefCell::new(None),
            window,
            notebook,
            _night_light: night_light,
            content,
            canvas,
            tabs,
            tab_buttons: RefCell::new(Vec::new()),
            select: RefCell::new(None),
            form,
            title,
            connector,
            enabled,
            enabled_note,
            primary,
            primary_note,
            resolution,
            rate,
            scale,
            scale_label,
            scale_note,
            rows_of_an_enabled_display,
            message,
            identify,
            reset,
            apply,
            scrim,
            countdown,
            revert,
            keep,
        }
    }

    /// Joins the widgets and the compositor to what they do.
    fn connect(self: &Rc<Self>) {
        let show_tab = gio::SimpleAction::new("show-tab", Some(glib::VariantTy::STRING));
        let notebook = self.notebook.clone();
        show_tab.connect_activate(move |_, name| {
            let tab = name.and_then(|name| name.str()).and_then(Tab::from_name);
            if let Some(tab) = tab {
                notebook.set_current_page(Some(tab.page()));
            }
        });
        self.window.add_action(&show_tab);

        let ui = self.clone();
        self.compositor.listen(move |event| ui.heard(event));

        let ui = self.clone();
        self.canvas.set_draw_func(move |area, cr, width, height| {
            let page = ui.page.borrow();
            let view = ui.view(&page, width, height);
            arrangement::draw(area, cr, &page, &view);
        });
        let drag = gtk::GestureDrag::new();
        let ui = self.clone();
        drag.connect_drag_begin(move |_, x, y| ui.pressed(x, y));
        let ui = self.clone();
        drag.connect_drag_update(move |_, dx, dy| ui.dragged(dx, dy));
        let ui = self.clone();
        drag.connect_drag_end(move |_, _, _| ui.released());
        self.canvas.add_controller(drag);

        // Each setting changes the selected display of the page, and the page is shown
        // again, since one setting moves the others: a resolution has its own rates.
        let ui = self.clone();
        self.enabled.connect_toggled(move |check| {
            ui.change(|page, index| page.set_enabled(index, check.is_active()));
        });
        let ui = self.clone();
        self.primary.connect_toggled(move |check| {
            if check.is_active() {
                ui.change(|page, index| page.set_primary(index));
            }
        });
        let ui = self.clone();
        self.resolution.connect_selected_notify(move |dropdown| {
            let chosen = dropdown.selected() as usize;
            ui.change(|page, index| {
                let resolutions = page.draft().displays[index].resolutions();
                if let Some(resolution) = resolutions.get(chosen) {
                    page.set_resolution(index, resolution.width, resolution.height);
                }
            });
        });
        let ui = self.clone();
        self.rate.connect_selected_notify(move |dropdown| {
            let chosen = dropdown.selected() as usize;
            ui.change(|page, index| {
                if let Some(&rate) = page.draft().displays[index].rates().get(chosen) {
                    page.set_rate(index, rate);
                }
            });
        });
        let ui = self.clone();
        self.scale.connect_value_changed(move |scale| {
            // The slider stops only at the scales the page offers.
            let offered = Scale::offered(scale.value());
            ui.change(|page, index| page.set_scale(index, offered));
        });

        // The entries of the row are made as the monitors come, and reach the window
        // through this.
        let ui = self.clone();
        self.select.replace(Some(Rc::new(move |index| {
            ui.page.borrow_mut().select(index);
            ui.render();
        })));

        let ui = self.clone();
        self.identify.connect_clicked(move |_| ui.identify());
        let ui = self.clone();
        self.reset.connect_clicked(move |_| {
            ui.page.borrow_mut().reset();
            ui.say("");
            ui.render();
        });
        let ui = self.clone();
        self.apply.connect_clicked(move |_| ui.apply());
        let ui = self.clone();
        self.keep.connect_clicked(move |_| ui.keep());
        let ui = self.clone();
        self.revert.connect_clicked(move |_| ui.revert());

        let ui = self.clone();
        self.window.connect_close_request(move |_| {
            ui.closing();
            glib::Propagation::Proceed
        });
    }

    /// Changes the selected display as a setting's widget says, unless it's the page
    /// that is setting the widget.
    fn change(&self, change: impl FnOnce(&mut Page, usize)) {
        if self.rendering.get() {
            return;
        }
        {
            let mut page = self.page.borrow_mut();
            if page.selected().is_none() {
                return;
            }
            let index = page.selected_index();
            change(&mut page, index);
        }
        self.render();
    }

    /// Shows `text` under the settings, or nothing for an empty one.
    fn say(&self, text: &str) {
        self.message.set_label(text);
        self.message.set_visible(!text.is_empty());
    }

    /// The view the arrangement is drawn in: the one a display was picked up in while
    /// it's in the air, else the one that fits the layout.
    fn view(&self, page: &Page, width: i32, height: i32) -> View {
        if let Some(Press { view, moved: true }) = self.press.get() {
            return view;
        }
        let rects: Vec<_> = page
            .arrangement()
            .into_iter()
            .map(|(_, rect)| rect)
            .collect();
        View::fit(&rects, f64::from(width), f64::from(height))
    }

    fn pressed(&self, x: f64, y: f64) {
        {
            let mut page = self.page.borrow_mut();
            let view = self.view(&page, self.canvas.width(), self.canvas.height());
            let arrangement = page.arrangement();
            // The selected display is drawn on top, so it's the one under the pointer
            // where two tiles' borders meet.
            let selected = page.selected_index();
            let on = |&&(_, rect): &&(usize, _)| view.hits(rect, x, y);
            let hit = arrangement
                .iter()
                .filter(|(index, _)| *index == selected)
                .chain(&arrangement)
                .find(on);
            let Some(&(index, _)) = hit else {
                return;
            };
            page.pick_up(index);
            self.press.set(Some(Press { view, moved: false }));
        }
        self.render();
    }

    fn dragged(&self, dx: f64, dy: f64) {
        let Some(mut press) = self.press.get() else {
            return;
        };
        if !press.moved && dx.hypot(dy) < arrangement::DRAG_THRESHOLD {
            return;
        }
        press.moved = true;
        self.press.set(Some(press));
        let view = press.view;
        self.page
            .borrow_mut()
            .drag_to(view.logical(dx), view.logical(dy), view.snap());
        self.canvas.queue_draw();
    }

    fn released(&self) {
        if self.press.take().is_some() {
            self.page.borrow_mut().put_down();
            self.render();
        }
    }

    /// Gives every widget what the page has.
    fn render(&self) {
        self.rendering.set(true);
        let page = self.page.borrow();
        let draft = page.draft();
        let selected = page.selected_index();

        self.render_tabs(&page);
        self.form.set_visible(page.selected().is_some());
        if let Some(display) = page.selected() {
            self.title.set_markup(&format!(
                "<b>{}</b>",
                glib::markup_escape_text(&display.title)
            ));
            self.connector.set_label(&display.name);

            self.enabled.set_active(display.enabled);
            let last = !page.can_turn_off(selected);
            self.enabled.set_sensitive(!last);
            self.enabled_note.set_label(if last {
                "The only display that is on can't be turned off."
            } else {
                ""
            });

            for row in &self.rows_of_an_enabled_display {
                row.set_sensitive(display.enabled);
            }
            let is_primary = draft.is_primary(selected);
            self.primary.set_active(is_primary);
            self.primary
                .set_sensitive(!is_primary && draft.primary.is_some());
            self.primary_note
                .set_visible_child_name(if draft.primary.is_none() {
                    "none"
                } else if is_primary {
                    "here"
                } else {
                    "elsewhere"
                });

            let shown = display.mode();
            let resolutions = display.resolutions();
            let labels: Vec<String> = resolutions.iter().map(|r| r.label()).collect();
            fill(&self.resolution, &labels);
            let of_shown = |r: &caminito_displays::page::Resolution| {
                shown.is_some_and(|mode| (r.width, r.height) == (mode.width, mode.height))
            };
            if let Some(at) = resolutions.iter().position(of_shown) {
                self.resolution.set_selected(at as u32);
            }

            let rates = display.rates();
            let labels: Vec<String> = rates.iter().map(|&rate| rate_label(rate)).collect();
            fill(&self.rate, &labels);
            let shown_rate = |&rate: &i32| shown.is_some_and(|mode| mode.refresh == rate);
            if let Some(at) = rates.iter().position(shown_rate) {
                self.rate.set_selected(at as u32);
            }

            self.scale.set_value(f64::from(display.scale.percent()));
            self.scale_label
                .set_markup(&format!("<b>{}</b>", display.scale.label()));
            let (width, height) = display.logical_size();
            self.scale_note
                .set_label(&format!("Looks like {width} × {height}"));
        }

        let editing = matches!(&*self.phase.borrow(), Phase::Editing);
        let lost = matches!(&*self.phase.borrow(), Phase::Lost);
        self.content.set_sensitive(editing);
        self.apply.set_sensitive(page.is_dirty());
        self.reset.set_sensitive(page.is_dirty());
        self.identify
            .set_sensitive(self.compositor.can_identify() && !lost);
        self.rendering.set(false);
        self.canvas.queue_draw();
    }

    /// The row with one entry per display. The entries stay while the monitors do, so
    /// that the one being pressed isn't taken away under the pointer.
    fn render_tabs(&self, page: &Page) {
        let displays = &page.draft().displays;
        let mut buttons = self.tab_buttons.borrow_mut();
        let same = buttons.len() == displays.len()
            && buttons
                .iter()
                .zip(displays)
                .all(|((name, _), d)| *name == d.name);
        if !same {
            for (_, button) in buttons.drain(..) {
                self.tabs.remove(&button);
            }
            for (index, display) in displays.iter().enumerate() {
                let button = gtk::ToggleButton::new();
                button.set_child(Some(&gtk::Label::new(None)));
                self.tabs.append(&button);
                buttons.push((display.name.clone(), button.clone()));
                if let Some(select) = self.select.borrow().clone() {
                    button.connect_clicked(move |_| select(index));
                }
            }
        }
        for (index, (display, (_, button))) in displays.iter().zip(buttons.iter()).enumerate() {
            let star = if page.draft().is_primary(index) {
                "★ "
            } else {
                ""
            };
            let state = if display.enabled {
                display.name.as_str()
            } else {
                "off"
            };
            let text = format!(
                "{star}{}  <span alpha=\"60%\">{}</span>",
                glib::markup_escape_text(&display.title),
                glib::markup_escape_text(state)
            );
            if let Some(label) = button.child().and_downcast::<gtk::Label>() {
                label.set_markup(&text);
            }
            button.set_active(index == page.selected_index());
        }
    }

    /// The compositor said something.
    fn heard(self: &Rc<Self>, event: Event) {
        match event {
            Event::Changed => self.changed(),
            Event::Applied(outcome) => self.applied(outcome),
            Event::Lost(why) => {
                self.stop_countdown();
                *self.phase.borrow_mut() = Phase::Lost;
                self.scrim.set_visible(false);
                self.say(&format!(
                    "The displays can't be changed from here any more: {why}."
                ));
                self.render();
            }
        }
    }

    /// Caminito shows something else. The window shows that, and what wasn't applied is
    /// dropped: a hot-plug, a configuration of this window's or of `wlr-randr`'s, or
    /// another primary output.
    fn changed(&self) {
        let monitors = self.compositor.monitors();
        let primary = self.compositor.primary();
        let same_monitors = {
            let mut page = self.page.borrow_mut();
            let before = page.applied().clone();
            // Caminito says again what it showed: its name's owner, a signal for no change.
            if !page.refresh(&monitors, primary.as_deref()) {
                return;
            }
            self.press.set(None);
            page.same_monitors(&before)
        };
        // What there was to go back to was a layout of other monitors. Caminito has laid
        // the new set out itself, from its saved layout if it has one (83).
        let confirming = matches!(&*self.phase.borrow(), Phase::Confirming { .. });
        if confirming && !same_monitors {
            self.stop_countdown();
            *self.phase.borrow_mut() = Phase::Editing;
            self.scrim.set_visible(false);
            self.say("The monitors changed, so the configuration was neither kept nor reverted.");
        }
        self.render();
    }

    /// Apply: nothing outside the window has changed until now.
    fn apply(self: &Rc<Self>) {
        self.say("");
        let (before, primary, configuration, only_primary) = {
            let page = self.page.borrow();
            if !page.is_dirty() {
                return;
            }
            let primary = page.chosen_primary().map(str::to_owned);
            let before = Before {
                layout: page.applied().clone(),
                primary: page.applied().primary.clone().filter(|_| primary.is_some()),
            };
            (
                before,
                primary,
                page.configuration(),
                page.only_primary_differs(),
            )
        };
        // The primary display alone moves no picture, so there's nothing to lose sight
        // of: it's applied and saved at once.
        if only_primary {
            let Some(primary) = primary else {
                return;
            };
            let ui = self.clone();
            self.compositor
                .choose_primary(&primary, move |chosen| match chosen {
                    Ok(()) => ui.save(),
                    Err(err) => ui.say(&format!("The primary display wasn't changed: {err}.")),
                });
            return;
        }
        match self.compositor.apply(&configuration) {
            Ok(()) => *self.phase.borrow_mut() = Phase::Applying { before, primary },
            Err(err) => self.say(&format!("The configuration wasn't applied: {err}.")),
        }
        self.render();
    }

    /// Asks Caminito to save the layout it shows.
    fn save(self: &Rc<Self>) {
        let ui = self.clone();
        self.compositor.save_layout(move |saved| {
            if let Err(err) = saved {
                ui.say(&format!(
                    "The configuration is applied, but it wasn't saved for the next session: {err}."
                ));
            }
        });
    }

    /// Chooses `primary` as the primary output, if there's one to choose, once Caminito
    /// shows the configuration that has its display on. With no choice made here,
    /// Caminito's own rule stands: its primary output is the built-in panel, else the
    /// leftmost (81), which a display that moved may change.
    fn choose_primary(self: &Rc<Self>, primary: Option<String>) {
        let Some(primary) = &primary else {
            return;
        };
        let ui = self.clone();
        self.compositor.choose_primary(primary, move |chosen| {
            if let Err(err) = chosen {
                ui.say(&format!("The primary display wasn't changed: {err}."));
            }
        });
    }

    /// Caminito answered a configuration.
    fn applied(self: &Rc<Self>, outcome: Outcome) {
        let phase = std::mem::replace(&mut *self.phase.borrow_mut(), Phase::Editing);
        match (phase, outcome) {
            (Phase::Applying { before, primary }, Outcome::Succeeded) => {
                self.choose_primary(primary);
                self.start_countdown(before);
            }
            (Phase::Applying { .. }, Outcome::Failed) => {
                self.say("Caminito can't show this configuration, and shows what it did.");
            }
            (Phase::Applying { .. }, Outcome::Cancelled) => {
                self.say("The monitors changed before the configuration was applied.");
            }
            (Phase::Reverting { primary }, Outcome::Succeeded) => self.choose_primary(primary),
            (Phase::Reverting { .. }, _) => {
                self.say("The displays couldn't be put back to how they were.");
            }
            // An answer nobody waits for any more.
            (phase, _) => *self.phase.borrow_mut() = phase,
        }
        self.render();
    }

    fn start_countdown(self: &Rc<Self>, before: Before) {
        let ui = self.clone();
        let tick = glib::timeout_add_seconds_local(1, move || ui.tick());
        *self.phase.borrow_mut() = Phase::Confirming {
            before,
            left: COUNTDOWN,
            tick,
        };
        self.show_countdown(COUNTDOWN);
        self.scrim.set_visible(true);
        self.keep.grab_focus();
    }

    fn show_countdown(&self, left: u32) {
        let seconds = if left == 1 { "second" } else { "seconds" };
        self.countdown.set_label(&format!(
            "The displays go back to how they were in {left} {seconds}."
        ));
    }

    /// A second of the countdown is over.
    fn tick(self: &Rc<Self>) -> glib::ControlFlow {
        let left = match &mut *self.phase.borrow_mut() {
            Phase::Confirming { left, .. } => {
                *left -= 1;
                *left
            }
            _ => return glib::ControlFlow::Break,
        };
        if left > 0 {
            self.show_countdown(left);
            return glib::ControlFlow::Continue;
        }
        // The timer ends with this turn, so it isn't there to remove.
        let phase = std::mem::replace(&mut *self.phase.borrow_mut(), Phase::Editing);
        if let Phase::Confirming { before, .. } = phase {
            self.go_back_to(before);
        }
        glib::ControlFlow::Break
    }

    /// Ends the countdown without an answer, if one runs.
    fn stop_countdown(&self) -> Option<Before> {
        let mut phase = self.phase.borrow_mut();
        if !matches!(&*phase, Phase::Confirming { .. }) {
            return None;
        }
        match std::mem::replace(&mut *phase, Phase::Editing) {
            Phase::Confirming { before, tick, .. } => {
                tick.remove();
                Some(before)
            }
            _ => None,
        }
    }

    fn keep(self: &Rc<Self>) {
        if self.stop_countdown().is_some() {
            self.scrim.set_visible(false);
            self.save();
            self.render();
        }
    }

    fn revert(self: &Rc<Self>) {
        if let Some(before) = self.stop_countdown() {
            self.go_back_to(before);
        }
    }

    /// Sends the configuration that shows what Caminito showed `before`.
    fn go_back_to(self: &Rc<Self>, before: Before) {
        self.scrim.set_visible(false);
        let configuration = before.layout.configuration(self.page.borrow().applied());
        match self.compositor.apply(&configuration) {
            Ok(()) => {
                *self.phase.borrow_mut() = Phase::Reverting {
                    primary: before.primary,
                }
            }
            Err(err) => self.say(&format!("The displays couldn't be put back: {err}.")),
        }
        self.render();
    }

    /// The window closes. A configuration that wasn't kept isn't: it's reverted, and
    /// Caminito is waited for, since no turn of the main loop comes after this.
    fn closing(&self) {
        self.compositor.end_identify();
        // An answer still on its way starts the countdown, which ends here.
        if matches!(&*self.phase.borrow(), Phase::Applying { .. }) {
            self.compositor.settle();
        }
        let Some(before) = self.stop_countdown() else {
            return;
        };
        let configuration = before.layout.configuration(self.page.borrow().applied());
        if let Err(err) = self.compositor.apply(&configuration) {
            glib::g_warning!("caminito-displays", "can't put the displays back: {err}");
        }
        // The primary output's display may be one the configuration turns back on.
        self.compositor.settle();
        if let Some(primary) = &before.primary {
            self.compositor.choose_primary_now(primary);
        }
    }

    /// Shows each display's name on the monitor itself, for a moment.
    fn identify(self: &Rc<Self>) {
        // What the monitors show, not what the window would have them show.
        let labels = identify::labels(self.page.borrow().applied());
        self.compositor.identify(&labels);
        if let Some(running) = self.identifying.take() {
            running.remove();
        }
        let ui = self.clone();
        let over = glib::timeout_add_local_once(identify::SHOWN_FOR, move || {
            ui.identifying.take();
            ui.compositor.end_identify();
        });
        self.identifying.replace(Some(over));
    }
}
