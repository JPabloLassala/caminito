//! The Key remaps page (CMN-189, ADR 0038), as the prototype's variant C: the Remap template
//! bar at the top, All applications and the app ids on the left, and on the right, for the
//! one chosen, its Remap group and the remaps in effect, each with where it comes from. A
//! change is written at once, as on the Hot corners page, and the page follows the keys when
//! they change from outside, as by `gsettings set`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use caminito_behaviour::key_remaps::{
    Combination, KeyRemapKeys, Remaps, Rule, Scope, Source, parse_target, target_text,
};
use caminito_behaviour::open_windows;
use caminito_behaviour::remap_templates::{self, Template, Templates};
use caminito_behaviour::system_shortcuts::SystemShortcuts;
use futures_channel::oneshot;
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::{DOMAIN, SPACING};

/// The space between a row's parts, in pixels.
const GAP: i32 = SPACING / 3;
/// The applications' list's size, in pixels.
const APPS: (i32, i32) = (240, 440);
/// The remaps' table's size, in pixels.
const RULES: (i32, i32) = (600, 320);
/// What a target's field says until one is typed.
const TARGET_HINT: &str = "Ctrl+T · Shift+Home, BackSpace · nothing";

/// The page's state and the widgets it redraws.
struct Page {
    keys: KeyRemapKeys,
    templates: Templates,
    shortcuts: SystemShortcuts,
    /// The remaps as last written or heard, so a change heard that the page made itself
    /// redraws nothing.
    remaps: RefCell<Remaps>,
    /// The templates as last listed, in the drop-down's order.
    listed: RefCell<Vec<Template>>,
    /// The app ids of the windows open when the page opened.
    open: Vec<String>,
    /// The app ids the user added that are in no group yet.
    added: RefCell<Vec<String>>,
    /// The app id chosen, or `None` for All applications.
    chosen: RefCell<Option<String>>,
    /// Each row of the list's app id, `None` for All applications'; the last row adds one.
    rows: RefCell<Vec<Option<String>>>,
    /// Whether the page is redrawing, when its widgets' signals are its own.
    redrawing: Cell<bool>,
    root: gtk::Box,
    dropdown: gtk::DropDown,
    status: gtk::Label,
    apps: gtk::ListBox,
    detail: gtk::Box,
}

/// Makes the page over `keys`.
pub fn build(keys: KeyRemapKeys) -> gtk::Widget {
    let (listed, problems) = Templates::installed().list();
    for problem in problems {
        glib::g_warning!(DOMAIN, "can't read a remap template: {problem}");
    }
    let open = open_windows::app_ids().unwrap_or_else(|err| {
        glib::g_warning!(DOMAIN, "can't list the open windows: {err}");
        Vec::new()
    });
    let page = Rc::new(Page {
        remaps: RefCell::new(keys.remaps()),
        keys,
        templates: Templates::installed(),
        shortcuts: SystemShortcuts::installed(),
        listed: RefCell::new(listed),
        open,
        added: RefCell::default(),
        chosen: RefCell::default(),
        rows: RefCell::default(),
        redrawing: Cell::new(false),
        root: gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(SPACING)
            .margin_top(SPACING)
            .margin_bottom(SPACING)
            .margin_start(SPACING)
            .margin_end(SPACING)
            .build(),
        dropdown: gtk::DropDown::builder().build(),
        status: gtk::Label::builder().hexpand(true).xalign(1.0).build(),
        apps: gtk::ListBox::new(),
        detail: gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(GAP)
            .hexpand(true)
            .build(),
    });

    page.root.append(&template_bar(&page));
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(SPACING)
        .build();
    body.append(
        &gtk::ScrolledWindow::builder()
            .child(&page.apps)
            .width_request(APPS.0)
            .height_request(APPS.1)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .has_frame(true)
            .build(),
    );
    body.append(&page.detail);
    page.root.append(&body);

    page.apps.connect_row_selected({
        let page = Rc::downgrade(&page);
        move |_, row| {
            let (Some(page), Some(row)) = (page.upgrade(), row) else {
                return;
            };
            if page.redrawing.get() {
                return;
            }
            let index = row.index() as usize;
            let chosen = page.rows.borrow().get(index).cloned();
            match chosen {
                Some(app_id) => {
                    *page.chosen.borrow_mut() = app_id;
                    page.redraw_detail();
                }
                None => {
                    glib::spawn_future_local(add_app_id(page));
                }
            }
        }
    });
    page.keys.connect_changed({
        let page = Rc::downgrade(&page);
        move || {
            let Some(page) = page.upgrade() else {
                return;
            };
            let remaps = page.keys.remaps();
            if remaps != *page.remaps.borrow() {
                *page.remaps.borrow_mut() = remaps;
                page.redraw();
            } else {
                page.redraw_status();
            }
        }
    });
    page.redraw();
    page.root.clone().upcast()
}

impl Page {
    /// The window the page is in, for its dialogs.
    fn window(&self) -> Option<gtk::Window> {
        self.root.root().and_downcast()
    }

    /// Writes `remaps` at once, and redraws the page for them.
    fn write(self: &Rc<Self>, remaps: Remaps) {
        match self.keys.set_remaps(&remaps) {
            Ok(()) => *self.remaps.borrow_mut() = remaps,
            Err(err) => {
                glib::g_warning!(DOMAIN, "can't set the key remaps: {err}");
                *self.remaps.borrow_mut() = self.keys.remaps();
            }
        }
        self.redraw();
    }

    /// Changes the remaps with `change`, and writes them.
    fn edit(self: &Rc<Self>, change: impl FnOnce(&mut Remaps)) {
        let mut remaps = self.remaps.borrow().clone();
        change(&mut remaps);
        if remaps != *self.remaps.borrow() {
            self.write(remaps);
        }
    }

    /// Writes the name of the template the remaps were loaded from or saved as.
    fn set_template(self: &Rc<Self>, name: &str) {
        if let Err(err) = self.keys.set_template(name) {
            glib::g_warning!(DOMAIN, "can't set the remap template: {err}");
        }
        self.redraw_status();
    }

    /// Lists the templates again, for the drop-down.
    fn relist(self: &Rc<Self>) {
        let (listed, problems) = self.templates.list();
        for problem in problems {
            glib::g_warning!(DOMAIN, "can't read a remap template: {problem}");
        }
        *self.listed.borrow_mut() = listed;
        self.redraw_status();
    }

    /// The template the drop-down shows.
    fn template_shown(&self) -> Option<Template> {
        let listed = self.listed.borrow();
        listed.get(self.dropdown.selected() as usize).cloned()
    }

    fn redraw(self: &Rc<Self>) {
        self.redraw_status();
        self.redraw_apps();
        self.redraw_detail();
    }

    /// The template bar's drop-down, and where the remaps came from.
    fn redraw_status(&self) {
        let name = self.keys.template();
        let listed = self.listed.borrow();
        let names: Vec<&str> = listed
            .iter()
            .map(|template| template.name.as_str())
            .collect();
        self.redrawing.set(true);
        self.dropdown.set_model(Some(&gtk::StringList::new(&names)));
        if let Some(index) = names.iter().position(|each| *each == name) {
            self.dropdown.set_selected(index as u32);
        }
        self.redrawing.set(false);
        let from = listed.iter().find(|template| template.name == name);
        let text = match from {
            _ if name.is_empty() => "Not loaded from a template".to_owned(),
            Some(template) if template.remaps == *self.remaps.borrow() => {
                format!("Loaded from <b>{}</b>", glib::markup_escape_text(&name))
            }
            _ => format!(
                "Loaded from <b>{}</b> · changed",
                glib::markup_escape_text(&name)
            ),
        };
        self.status.set_markup(&text);
    }

    /// The list: All applications, then the app ids of the groups, of the open windows and
    /// the user's, each with its group.
    fn redraw_apps(&self) {
        let remaps = self.remaps.borrow();
        let mut app_ids: Vec<String> = remaps
            .groups
            .iter()
            .flat_map(|group| group.app_ids.iter().cloned())
            .chain(self.open.iter().cloned())
            .chain(self.added.borrow().iter().cloned())
            .collect();
        app_ids.sort_by_key(|app_id| app_id.to_lowercase());
        app_ids.dedup();

        self.redrawing.set(true);
        self.apps.remove_all();
        let count = remaps.all.len();
        self.apps.append(&item(
            "<b>All applications</b>",
            &format!("{count} remap{}", if count == 1 { "" } else { "s" }),
        ));
        for app_id in &app_ids {
            let group = remaps.group_of(app_id).map(|index| &remaps.groups[index]);
            let group = match group {
                Some(group) if group.no_remaps => format!("{} · no remaps", group.name),
                Some(group) => group.name.clone(),
                None => "No group".to_owned(),
            };
            self.apps
                .append(&item(&glib::markup_escape_text(app_id), &group));
        }
        self.apps.append(&item("Another app id…", ""));
        let mut rows: Vec<Option<Option<String>>> = std::iter::once(Some(None))
            .chain(app_ids.into_iter().map(|app_id| Some(Some(app_id))))
            .collect();
        rows.push(None);
        let chosen = self.chosen.borrow().clone();
        let selected = rows.iter().position(|row| row.as_ref() == Some(&chosen));
        self.apps.select_row(
            self.apps
                .row_at_index(selected.unwrap_or(0) as i32)
                .as_ref(),
        );
        *self.rows.borrow_mut() = rows.into_iter().map(Option::flatten).collect();
        self.redrawing.set(false);
    }

    /// The right side, for All applications or the app id chosen.
    fn redraw_detail(self: &Rc<Self>) {
        while let Some(child) = self.detail.first_child() {
            self.detail.remove(&child);
        }
        let chosen = self.chosen.borrow().clone();
        match chosen {
            None => self.all_applications(),
            Some(app_id) => self.application(&app_id),
        }
    }

    /// All applications' remaps, each changeable, and a row to add one.
    fn all_applications(self: &Rc<Self>) {
        self.detail.append(&heading("All applications", None));
        self.detail.append(&note(
            "These apply to every application, unless its group changes them or has no \
             remaps. A combination that is a System shortcut is never remapped.",
        ));
        let table = Table::new(&["When you press", "The application receives", "", ""]);
        let mut rules = self.remaps.borrow().all.clone();
        rules.sort_by_key(|rule| rule.combination.to_string());
        for rule in rules {
            table.row([
                combination_label(&rule.combination).upcast(),
                self.target_entry(Scope::AllApplications, &rule).upcast(),
                self.marks(&rule).upcast(),
                self.remove_button(Scope::AllApplications, &rule).upcast(),
            ]);
        }
        self.add_row(&table, Scope::AllApplications);
        self.detail.append(&table.scrolled());
    }

    /// `app_id`'s group, and the remaps in effect for it.
    fn application(self: &Rc<Self>, app_id: &str) {
        let remaps = self.remaps.borrow().clone();
        let group = remaps.group_of(app_id);
        let header = heading(app_id, Some(self.group_dropdown(app_id, &remaps, group)));
        if let Some(index) = group {
            header.append(&self.no_remaps_switch(index, remaps.groups[index].no_remaps));
        }
        self.detail.append(&header);

        let Some(index) = group else {
            self.detail.append(&note(
                "Not in a group: All applications' remaps apply. Put it in a group to change \
                 them for it.",
            ));
            self.detail.append(&self.in_effect(app_id, &remaps, None));
            return;
        };
        let group = &remaps.groups[index];
        let others: Vec<&str> = group
            .app_ids
            .iter()
            .map(String::as_str)
            .filter(|each| *each != app_id)
            .collect();
        let others = if others.is_empty() {
            "no other app id".to_owned()
        } else {
            others.join(", ")
        };
        self.detail.append(&note(&format!(
            "Shared with {others}. Grey rows come from All applications."
        )));
        if group.no_remaps {
            self.detail.append(
                &gtk::Label::builder()
                    .label(format!(
                        "Every combination reaches {app_id} as it's pressed."
                    ))
                    .margin_top(SPACING)
                    .build(),
            );
            return;
        }
        self.detail
            .append(&self.in_effect(app_id, &remaps, Some(index)));
    }

    /// The table of the remaps in effect for `app_id`, in `group` if it's in one: its
    /// group's own changeable, All applications' grey with "Change for <group>".
    fn in_effect(
        self: &Rc<Self>,
        app_id: &str,
        remaps: &Remaps,
        group: Option<usize>,
    ) -> gtk::ScrolledWindow {
        let receives = format!("{app_id} receives");
        let table = Table::new(&["When you press", &receives, "From", ""]);
        for row in remaps.in_effect(Some(app_id)) {
            let rule = &row.rule;
            let target: gtk::Widget = match row.source {
                Source::Group(index) => self.target_entry(Scope::Group(index), rule).upcast(),
                Source::AllApplications => gtk::Label::builder()
                    .label(target_text(&rule.steps))
                    .xalign(0.0)
                    .build()
                    .upcast(),
            };
            let from = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let source = match row.source {
                Source::AllApplications => "All applications",
                Source::Group(index) => &remaps.groups[index].name,
            };
            from.append(&gtk::Label::builder().label(source).xalign(0.0).build());
            if let Some(replaced) = &row.replaces {
                from.append(&small(&format!("replaces {}", target_text(replaced))));
            }
            from.append(&self.marks(rule));
            let action: gtk::Widget = match (row.source, group) {
                (Source::Group(index), _) => self.remove_button(Scope::Group(index), rule).upcast(),
                (Source::AllApplications, Some(index)) => self
                    .change_for_button(index, &remaps.groups[index].name, rule)
                    .upcast(),
                (Source::AllApplications, None) => {
                    gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast()
                }
            };
            let cells = [
                combination_label(&rule.combination).upcast(),
                target,
                from.upcast(),
                action,
            ];
            let inherited = row.source == Source::AllApplications && group.is_some();
            if inherited {
                // The button stays readable: it's how the row is changed.
                for cell in &cells[..3] {
                    cell.add_css_class("dim-label");
                }
            }
            table.row(cells);
        }
        if let Some(index) = group {
            self.add_row(&table, Scope::Group(index));
        }
        table.scrolled()
    }

    /// The drop-down of `app_id`'s group: None, each group, and New group….
    fn group_dropdown(
        self: &Rc<Self>,
        app_id: &str,
        remaps: &Remaps,
        group: Option<usize>,
    ) -> gtk::Box {
        let mut labels = vec!["None: All applications only".to_owned()];
        labels.extend(remaps.groups.iter().map(|group| group.name.clone()));
        labels.push("New group…".to_owned());
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let dropdown = gtk::DropDown::builder()
            .model(&gtk::StringList::new(&labels))
            .selected(group.map_or(0, |index| index as u32 + 1))
            .build();
        let groups = remaps.groups.len() as u32;
        dropdown.connect_selected_notify({
            let page = Rc::downgrade(self);
            let app_id = app_id.to_owned();
            move |dropdown| {
                let Some(page) = page.upgrade() else {
                    return;
                };
                match dropdown.selected() {
                    0 => page.edit(|remaps| remaps.move_app_id(&app_id, None)),
                    new if new > groups => {
                        glib::spawn_future_local(new_group(page, app_id.clone()));
                    }
                    index => page.edit(|remaps| {
                        remaps.move_app_id(&app_id, Some(index as usize - 1));
                    }),
                }
            }
        });
        let row = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
        row.append(
            &gtk::Label::builder()
                .label("_Group:")
                .use_underline(true)
                .mnemonic_widget(&dropdown)
                .build(),
        );
        row.append(&dropdown);
        row
    }

    /// The group's "No remaps" switch.
    fn no_remaps_switch(self: &Rc<Self>, index: usize, no_remaps: bool) -> gtk::Box {
        let switch = gtk::Switch::builder()
            .active(no_remaps)
            .valign(gtk::Align::Center)
            .build();
        switch.connect_active_notify({
            let page = Rc::downgrade(self);
            move |switch| {
                if let Some(page) = page.upgrade() {
                    let active = switch.is_active();
                    page.edit(|remaps| remaps.set_no_remaps(index, active));
                }
            }
        });
        let row = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
        row.append(
            &gtk::Label::builder()
                .label("_No remaps")
                .use_underline(true)
                .mnemonic_widget(&switch)
                .build(),
        );
        row.append(&switch);
        row
    }

    /// The field of `rule`'s target in `scope`, written when Enter is pressed or the focus
    /// leaves it, or marked if it can't be read.
    fn target_entry(self: &Rc<Self>, scope: Scope, rule: &Rule) -> gtk::Entry {
        let entry = gtk::Entry::builder()
            .text(target_text(&rule.steps))
            .placeholder_text(TARGET_HINT)
            .hexpand(true)
            .build();
        let commit = {
            let page = Rc::downgrade(self);
            let combination = rule.combination.clone();
            move |entry: &gtk::Entry| {
                let Some(page) = page.upgrade() else {
                    return;
                };
                // A field the page is taking away loses its focus, which writes nothing.
                if page.redrawing.get() || entry.parent().is_none() {
                    return;
                }
                match parse_target(&entry.text()) {
                    Ok(steps) => {
                        entry.remove_css_class("error");
                        entry.set_tooltip_text(None);
                        let rule = Rule {
                            combination: combination.clone(),
                            steps,
                        };
                        page.edit(|remaps| {
                            if let Scope::Group(index) = scope
                                && index >= remaps.groups.len()
                            {
                                return;
                            }
                            remaps.set_rule(scope, rule);
                        });
                    }
                    Err(err) => {
                        entry.add_css_class("error");
                        entry.set_tooltip_text(Some(&err.to_string()));
                    }
                }
            }
        };
        entry.connect_activate(commit.clone());
        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave({
            let entry = entry.downgrade();
            move |_| {
                if let Some(entry) = entry.upgrade() {
                    commit(&entry);
                }
            }
        });
        entry.add_controller(focus);
        entry
    }

    /// The ✕ that removes `rule` from `scope`.
    fn remove_button(self: &Rc<Self>, scope: Scope, rule: &Rule) -> gtk::Button {
        let button = gtk::Button::builder()
            .icon_name("edit-delete-symbolic")
            .tooltip_text(format!("Remove {}", rule.combination))
            .has_frame(false)
            .build();
        let page = Rc::downgrade(self);
        let combination = rule.combination.clone();
        button.connect_clicked(move |_| {
            if let Some(page) = page.upgrade() {
                page.edit(|remaps| remaps.remove_rule(scope, &combination));
            }
        });
        button
    }

    /// "Change for <group>", which adds All applications' `rule` to the group at `index`.
    fn change_for_button(self: &Rc<Self>, index: usize, name: &str, rule: &Rule) -> gtk::Button {
        let button = gtk::Button::builder()
            .label(format!("Change for {name}"))
            .has_frame(false)
            .build();
        let page = Rc::downgrade(self);
        let combination = rule.combination.clone();
        button.connect_clicked(move |_| {
            if let Some(page) = page.upgrade() {
                page.edit(|remaps| remaps.change_for(index, &combination));
            }
        });
        button
    }

    /// The last row of `table`: a combination entered by pressing it, a target typed, and
    /// Add, which adds the rule to `scope`.
    fn add_row(self: &Rc<Self>, table: &Table, scope: Scope) {
        let (capture, pressed) = capture_button();
        let target = gtk::Entry::builder()
            .placeholder_text(TARGET_HINT)
            .hexpand(true)
            .build();
        let add = gtk::Button::with_label("Add");
        add.add_css_class("suggested-action");
        let add_rule = {
            let page = Rc::downgrade(self);
            move |target: &gtk::Entry| {
                let (Some(page), Some(combination)) = (page.upgrade(), pressed.borrow().clone())
                else {
                    return;
                };
                match parse_target(&target.text()) {
                    Ok(steps) => {
                        page.edit(|remaps| remaps.set_rule(scope, Rule { combination, steps }))
                    }
                    Err(err) => {
                        target.add_css_class("error");
                        target.set_tooltip_text(Some(&err.to_string()));
                    }
                }
            }
        };
        add.connect_clicked({
            let target = target.clone();
            let add_rule = add_rule.clone();
            move |_| add_rule(&target)
        });
        target.connect_activate(add_rule);
        table.row([
            capture.upcast(),
            target.upcast(),
            add.upcast(),
            gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
        ]);
    }

    /// What `rule` says of System shortcuts: that it never applies, its combination being
    /// one, and which of its steps run one.
    fn marks(&self, rule: &Rule) -> gtk::Box {
        let marks = gtk::Box::new(gtk::Orientation::Vertical, 0);
        if let Some(shortcut) = self.shortcuts.named(&rule.combination) {
            let mark = small(&format!("Never applies: the System shortcut “{shortcut}”"));
            mark.add_css_class("error");
            marks.append(&mark);
        }
        for step in &rule.steps {
            if let Some(shortcut) = self.shortcuts.named(step) {
                marks.append(&small(&format!("{step} runs “{shortcut}”")));
            }
        }
        marks
    }
}

/// A combination, pressed into a button: the button listens once clicked, and shows what was
/// pressed. Escape alone stops it listening.
fn capture_button() -> (gtk::Button, Rc<RefCell<Option<Combination>>>) {
    const PROMPT: &str = "Press keys…";
    let button = gtk::Button::with_label(PROMPT);
    let pressed: Rc<RefCell<Option<Combination>>> = Rc::default();
    let listening = Rc::new(Cell::new(false));
    button.connect_clicked({
        let listening = listening.clone();
        move |button| {
            listening.set(true);
            button.set_label("Press a combination");
            button.grab_focus();
        }
    });
    let keys = gtk::EventControllerKey::new();
    // Before the window's Escape, and before the button takes Space or Enter.
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let button = button.downgrade();
        let pressed = pressed.clone();
        move |keys, key, keycode, state| {
            let Some(button) = button.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !listening.get() {
                return glib::Propagation::Proceed;
            }
            let event = keys
                .current_event()
                .and_then(|event| event.downcast::<gdk::KeyEvent>().ok());
            if event.as_ref().is_some_and(gdk::KeyEvent::is_modifier) {
                return glib::Propagation::Stop;
            }
            let held = |mask| state.contains(mask);
            let ctrl = held(gdk::ModifierType::CONTROL_MASK);
            let alt = held(gdk::ModifierType::ALT_MASK);
            let shift = held(gdk::ModifierType::SHIFT_MASK);
            let super_ = held(gdk::ModifierType::SUPER_MASK);
            listening.set(false);
            if key == gdk::Key::Escape && !(ctrl || alt || shift || super_) {
                let label = pressed.borrow().as_ref().map(Combination::to_string);
                button.set_label(label.as_deref().unwrap_or(PROMPT));
                return glib::Propagation::Stop;
            }
            let layout = event.map_or(0, |event| event.layout());
            let name = first_level(&button.display(), keycode, layout)
                .or_else(|| key.to_lower().name().map(Into::into));
            let Some(name) = name else {
                button.set_label(PROMPT);
                return glib::Propagation::Stop;
            };
            let combination = Combination::new(ctrl, alt, shift, super_, &name);
            button.set_label(&combination.to_string());
            *pressed.borrow_mut() = Some(combination);
            glib::Propagation::Stop
        }
    });
    button.add_controller(keys);
    (button, pressed)
}

/// The name of the keysym `keycode` gives without modifiers in `layout`, which is what
/// Caminito matches a rule's key by (ADR 0038).
fn first_level(display: &gdk::Display, keycode: u32, layout: u32) -> Option<String> {
    let keys = display.map_keycode(keycode)?;
    let (_, key) = keys
        .iter()
        .find(|(each, _)| each.group() == layout as i32 && each.level() == 0)?;
    key.name().map(Into::into)
}

/// Asks for the remaps in use to be saved before a template replaces them, if no template
/// holds them, then for the template to be loaded, and loads it.
async fn load(page: Rc<Page>) {
    let Some(template) = page.template_shown() else {
        return;
    };
    let window = page.window();
    let current = page.remaps.borrow().clone();
    let held = remap_templates::holding(&page.listed.borrow(), &current).is_some();
    if !held {
        let choice = choose(
            window.as_ref(),
            "Save the remaps in use first?",
            "They aren't saved in any template, and loading replaces them.",
            &["Cancel", "Don't Save", "Save As…"],
        )
        .await;
        match choice {
            Some(1) => {}
            Some(2) if save_as(page.clone()).await => {}
            _ => return,
        }
    }
    let replace = choose(
        window.as_ref(),
        &format!("Load “{}”?", template.name),
        "It replaces the remaps in use.",
        &["Cancel", "Load"],
    )
    .await;
    if replace != Some(1) {
        return;
    }
    page.write(template.remaps.clone());
    page.set_template(&template.name);
}

/// Asks for a name and saves the remaps in use as the user's template of it, and returns
/// whether it saved them. A name of one of Caminito's templates asks for another.
async fn save_as(page: Rc<Page>) -> bool {
    let window = page.window();
    let mut name = page.keys.template();
    loop {
        let Some(asked) = ask_text(window.as_ref(), "Save the remaps in use as:", &name).await
        else {
            return false;
        };
        name = asked.trim().to_owned();
        let existing = page
            .listed
            .borrow()
            .iter()
            .find(|template| template.name.eq_ignore_ascii_case(&name))
            .cloned();
        match existing {
            Some(template) if template.shipped => {
                choose(
                    window.as_ref(),
                    &format!("“{}” comes with Caminito", template.name),
                    "It can't be changed. Save the remaps under another name.",
                    &["OK"],
                )
                .await;
                name = format!("{name} (mine)");
                continue;
            }
            Some(template) => {
                let replace = choose(
                    window.as_ref(),
                    &format!("Replace your template “{}”?", template.name),
                    "",
                    &["Cancel", "Replace"],
                )
                .await;
                if replace != Some(1) {
                    return false;
                }
            }
            None => {}
        }
        let remaps = page.remaps.borrow().clone();
        return match page.templates.save_as(&name, &remaps) {
            Ok(saved) => {
                page.relist();
                page.set_template(&saved.name);
                true
            }
            Err(err) => {
                choose(
                    window.as_ref(),
                    "The template can't be saved",
                    &err.to_string(),
                    &["OK"],
                )
                .await;
                false
            }
        };
    }
}

/// Deletes the user's template the drop-down shows, once asked.
async fn delete(page: Rc<Page>) {
    let Some(template) = page.template_shown() else {
        return;
    };
    let window = page.window();
    if template.shipped {
        let detail = "Only your own templates can be deleted.";
        let message = format!("“{}” comes with Caminito", template.name);
        choose(window.as_ref(), &message, detail, &["OK"]).await;
        return;
    }
    let message = format!("Delete your template “{}”?", template.name);
    if choose(window.as_ref(), &message, "", &["Cancel", "Delete"]).await != Some(1) {
        return;
    }
    if let Err(err) = page.templates.delete(&template) {
        choose(
            window.as_ref(),
            "The template can't be deleted",
            &err.to_string(),
            &["OK"],
        )
        .await;
    }
    page.relist();
}

/// Asks for an app id the list hasn't, and chooses it.
async fn add_app_id(page: Rc<Page>) {
    let asked = ask_text(page.window().as_ref(), "Another app id:", "").await;
    match asked.map(|app_id| app_id.trim().to_owned()) {
        Some(app_id) if !app_id.is_empty() => {
            page.added.borrow_mut().push(app_id.clone());
            *page.chosen.borrow_mut() = Some(app_id);
        }
        _ => {}
    }
    page.redraw();
}

/// Asks for a new group's name, and puts `app_id` in it.
async fn new_group(page: Rc<Page>, app_id: String) {
    let asked = ask_text(page.window().as_ref(), "The new group's name:", "").await;
    match asked.map(|name| name.trim().to_owned()) {
        Some(name) if !name.is_empty() => page.edit(|remaps| {
            let index = remaps.add_group(&name);
            remaps.move_app_id(&app_id, Some(index));
        }),
        // The drop-down goes back to the group the app id is in.
        _ => page.redraw_detail(),
    }
}

/// Asks `message` with `buttons`, the first of which cancels and the last of which is the
/// default, and returns the one chosen.
async fn choose(
    window: Option<&gtk::Window>,
    message: &str,
    detail: &str,
    buttons: &[&str],
) -> Option<i32> {
    let dialog = gtk::AlertDialog::builder()
        .message(message)
        .detail(detail)
        .buttons(buttons)
        .cancel_button(0)
        .default_button(buttons.len() as i32 - 1)
        .modal(true)
        .build();
    dialog.choose_future(window).await.ok()
}

/// Asks for a line of text, with `initial` in its field, and returns it, or `None` if the
/// user cancelled.
async fn ask_text(window: Option<&gtk::Window>, label: &str, initial: &str) -> Option<String> {
    let dialog = gtk::Window::builder()
        .modal(true)
        .resizable(false)
        .title("Key remaps")
        .build();
    dialog.set_transient_for(window);
    let entry = gtk::Entry::builder()
        .text(initial)
        .activates_default(true)
        .width_chars(32)
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let ok = gtk::Button::with_label("OK");
    ok.add_css_class("suggested-action");
    let buttons = gtk::Box::builder()
        .spacing(GAP)
        .halign(gtk::Align::End)
        .build();
    buttons.append(&cancel);
    buttons.append(&ok);
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(GAP)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    content.append(&gtk::Label::builder().label(label).xalign(0.0).build());
    content.append(&entry);
    content.append(&buttons);
    dialog.set_child(Some(&content));
    dialog.set_default_widget(Some(&ok));

    let (send, answer) = oneshot::channel();
    let send = Rc::new(RefCell::new(Some(send)));
    let answer_with = {
        let dialog = dialog.downgrade();
        move |text: Option<String>| {
            if let Some(send) = send.borrow_mut().take() {
                // The receiver is awaited below, as long as the dialog is open.
                let _ = send.send(text);
            }
            if let Some(dialog) = dialog.upgrade() {
                dialog.close();
            }
        }
    };
    ok.connect_clicked({
        let entry = entry.clone();
        let answer_with = answer_with.clone();
        move |_| answer_with(Some(entry.text().into()))
    });
    cancel.connect_clicked({
        let answer_with = answer_with.clone();
        move |_| answer_with(None)
    });
    dialog.connect_close_request(move |_| {
        answer_with(None);
        glib::Propagation::Proceed
    });
    dialog.present();
    answer.await.ok().flatten()
}

/// What a button of the template bar runs.
type Action = fn(Rc<Page>) -> std::pin::Pin<Box<dyn Future<Output = ()>>>;

/// The template bar: the drop-down of the templates, Load, Save as…, Delete, and where the
/// remaps came from.
fn template_bar(page: &Rc<Page>) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
    bar.append(
        &gtk::Label::builder()
            .label("_Template:")
            .use_underline(true)
            .mnemonic_widget(&page.dropdown)
            .build(),
    );
    bar.append(&page.dropdown);
    let button = |label: &str, run: Action| {
        let button = gtk::Button::with_mnemonic(label);
        let page = Rc::downgrade(page);
        button.connect_clicked(move |_| {
            if let Some(page) = page.upgrade() {
                glib::spawn_future_local(run(page));
            }
        });
        bar.append(&button);
    };
    button("_Load", |page| Box::pin(load(page)));
    button("_Save As…", |page| {
        Box::pin(async move {
            save_as(page).await;
        })
    });
    button("_Delete", |page| Box::pin(delete(page)));
    bar.append(&page.status);
    bar
}

/// A row of the applications' list: a title in markup, and a line below it.
fn item(title: &str, below: &str) -> gtk::ListBoxRow {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_top(GAP / 2)
        .margin_bottom(GAP / 2)
        .margin_start(GAP)
        .margin_end(GAP)
        .build();
    content.append(
        &gtk::Label::builder()
            .label(title)
            .use_markup(true)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build(),
    );
    if !below.is_empty() {
        let below = small(below);
        below.add_css_class("dim-label");
        content.append(&below);
    }
    gtk::ListBoxRow::builder().child(&content).build()
}

/// The right side's first row: a title, and `rest` at its end.
fn heading(title: &str, rest: Option<gtk::Box>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, SPACING);
    row.append(
        &gtk::Label::builder()
            .label(format!("<b>{}</b>", glib::markup_escape_text(title)))
            .use_markup(true)
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    if let Some(rest) = rest {
        row.append(&rest);
    }
    row
}

/// A line that explains the right side.
fn note(text: &str) -> gtk::Label {
    let note = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .max_width_chars(70)
        .build();
    note.add_css_class("dim-label");
    note
}

/// A line in small type.
fn small(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(format!("<small>{}</small>", glib::markup_escape_text(text)))
        .use_markup(true)
        .xalign(0.0)
        .build()
}

/// A combination, as a key cap.
fn combination_label(combination: &Combination) -> gtk::Label {
    gtk::Label::builder()
        .label(format!(
            "<tt>{}</tt>",
            glib::markup_escape_text(&combination.to_string())
        ))
        .use_markup(true)
        .xalign(0.0)
        .build()
}

/// A table of four columns, under a header.
struct Table {
    grid: gtk::Grid,
    rows: Cell<i32>,
}

impl Table {
    fn new(header: &[&str; 4]) -> Self {
        let grid = gtk::Grid::builder()
            .row_spacing(GAP)
            .column_spacing(SPACING)
            .margin_top(GAP)
            .margin_bottom(GAP)
            .margin_start(GAP)
            .margin_end(GAP)
            .build();
        for (column, title) in header.iter().enumerate() {
            let title = gtk::Label::builder()
                .label(format!("<b>{}</b>", glib::markup_escape_text(title)))
                .use_markup(true)
                .xalign(0.0)
                .build();
            grid.attach(&title, column as i32, 0, 1, 1);
        }
        Table {
            grid,
            rows: Cell::new(1),
        }
    }

    fn row(&self, cells: [gtk::Widget; 4]) {
        let row = self.rows.get();
        for (column, cell) in cells.iter().enumerate() {
            cell.set_valign(gtk::Align::Center);
            self.grid.attach(cell, column as i32, row, 1, 1);
        }
        self.rows.set(row + 1);
    }

    fn scrolled(&self) -> gtk::ScrolledWindow {
        gtk::ScrolledWindow::builder()
            .child(&self.grid)
            .width_request(RULES.0)
            .height_request(RULES.1)
            .vexpand(true)
            .has_frame(true)
            .build()
    }
}
