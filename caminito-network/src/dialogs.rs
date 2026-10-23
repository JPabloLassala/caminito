//! The secret agent's dialogs (ADR 0041), in GTK4: a small window for each secret
//! NetworkManager asks for, as many at once as it has requests. The agent decides when one
//! opens and closes; this draws them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use caminito_network::agent::{Answer, Dialogs, Field, Medium, Question, Reply, Request};
use gtk::glib;
use gtk::prelude::*;

/// The dialogs, drawn as GTK windows.
#[derive(Default)]
pub(crate) struct GtkDialogs {
    /// The open dialogs, by their requests' numbers.
    open: Rc<RefCell<HashMap<u64, gtk::Window>>>,
}

impl Dialogs for GtkDialogs {
    fn ask(&self, request: Request, reply: Reply) {
        let id = reply.id();
        let reply = Rc::new(RefCell::new(Some(reply)));
        let title = match &request.question {
            Question::Vpn { title, .. } if !title.is_empty() => title.as_str(),
            _ => "Network Authentication",
        };
        let window = gtk::Window::builder()
            .title(title)
            .resizable(false)
            .default_width(380)
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(18);
        content.set_margin_bottom(18);
        content.set_margin_start(18);
        content.set_margin_end(18);
        window.set_child(Some(&content));

        content.append(&label(&text(&request)));

        let form = match &request.question {
            Question::Vpn { fields, .. } => vpn_form(&content, fields),
            question => secret_form(&content, question),
        };

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label("Cancel");
        let connect = gtk::Button::with_label("Connect");
        connect.add_css_class("suggested-action");
        buttons.append(&cancel);
        buttons.append(&connect);
        content.append(&buttons);

        let answer = form.answer;
        let validate = {
            let answer = answer.clone();
            let connect = connect.clone();
            let question = request.question.clone();
            move || connect.set_sensitive(question.accepts(&answer()))
        };
        validate();
        let activate = {
            let connect = connect.clone();
            move || {
                if connect.is_sensitive() {
                    connect.emit_clicked();
                }
            }
        };
        for entry in &form.entries {
            on_edit(entry, validate.clone(), activate.clone());
        }

        connect.connect_clicked({
            let reply = reply.clone();
            let window = window.clone();
            move |_| {
                if let Some(reply) = reply.borrow_mut().take() {
                    reply.send(answer());
                }
                window.close();
            }
        });
        cancel.connect_clicked({
            let window = window.clone();
            move |_| window.close()
        });
        // Closing it any way the user can is a cancel. When the answer was sent already, or
        // the agent closed the dialog, there's no reply left or the agent ignores it.
        window.connect_close_request({
            let open = Rc::downgrade(&self.open);
            move |_| {
                if let Some(reply) = reply.borrow_mut().take() {
                    reply.cancel();
                }
                if let Some(open) = open.upgrade() {
                    open.borrow_mut().remove(&id);
                }
                glib::Propagation::Proceed
            }
        });

        form.focus.grab_focus();
        window.present();
        self.open.borrow_mut().insert(id, window);
    }

    fn close(&self, id: u64) {
        let window = self.open.borrow_mut().remove(&id);
        if let Some(window) = window {
            window.close();
        }
    }
}

/// A dialog's entries, and the answer they make.
struct Form {
    /// Each an entry or a password entry.
    entries: Vec<gtk::Widget>,
    /// The entry to start in.
    focus: gtk::Widget,
    answer: Rc<dyn Fn() -> Answer>,
}

/// The entry of a Wi-Fi or 802.1X secret, with the identity before it for an 802.1X
/// password.
fn secret_form(content: &gtk::Box, question: &Question) -> Form {
    let identity = match question {
        Question::Password { identity } => {
            let entry = gtk::Entry::builder()
                .text(identity.as_str())
                .placeholder_text("Username")
                .build();
            content.append(&entry);
            Some(entry)
        }
        _ => None,
    };
    let secret = gtk::PasswordEntry::builder()
        .show_peek_icon(true)
        .placeholder_text(placeholder(question))
        .build();
    content.append(&secret);

    let mut entries = vec![secret.clone().upcast::<gtk::Widget>()];
    entries.extend(identity.iter().map(|entry| entry.clone().upcast()));
    // An 802.1X identity that's there already only wants its password.
    let focus = match identity
        .as_ref()
        .filter(|identity| identity.text().is_empty())
    {
        Some(identity) => identity.clone().upcast(),
        None => secret.clone().upcast(),
    };
    let answer = move || {
        let text = secret.text().to_string();
        match &identity {
            Some(identity) => Answer::Login {
                identity: identity.text().to_string(),
                password: text,
            },
            None => Answer::Secret(text),
        }
    };
    Form {
        entries,
        focus,
        answer: Rc::new(answer),
    }
}

/// The fields a VPN's auth-dialog asks, each with its label, in a grid.
fn vpn_form(content: &gtk::Box, fields: &[Field]) -> Form {
    let grid = gtk::Grid::builder()
        .row_spacing(6)
        .column_spacing(12)
        .build();
    let entries: Vec<gtk::Widget> = fields
        .iter()
        .enumerate()
        .map(|(row, field)| {
            let name = gtk::Label::new(Some(&field.label));
            name.set_xalign(0.0);
            let entry: gtk::Widget = if field.secret {
                gtk::PasswordEntry::builder()
                    .show_peek_icon(true)
                    .text(field.value.as_str())
                    .hexpand(true)
                    .build()
                    .upcast()
            } else {
                gtk::Entry::builder()
                    .text(field.value.as_str())
                    .hexpand(true)
                    .build()
                    .upcast()
            };
            name.set_mnemonic_widget(Some(&entry));
            let row = i32::try_from(row).expect("a VPN asks a few fields");
            grid.attach(&name, 0, row, 1, 1);
            grid.attach(&entry, 1, row, 1, 1);
            entry
        })
        .collect();
    content.append(&grid);

    // The first that's empty, as a code, else the first.
    let focus = entries
        .iter()
        .find(|entry| entry_text(entry).is_empty())
        .or(entries.first())
        .cloned()
        .unwrap_or_else(|| grid.clone().upcast());
    let answer = {
        let entries = entries.clone();
        move || Answer::Fields(entries.iter().map(entry_text).collect())
    };
    Form {
        entries,
        focus,
        answer: Rc::new(answer),
    }
}

fn entry_text(entry: &gtk::Widget) -> String {
    entry
        .downcast_ref::<gtk::Editable>()
        .map(|editable| editable.text().to_string())
        .unwrap_or_default()
}

/// Calls `changed` when the entry's text changes, and `activate` on Enter in it.
fn on_edit(entry: &gtk::Widget, changed: impl Fn() + 'static, activate: impl Fn() + 'static) {
    if let Some(entry) = entry.downcast_ref::<gtk::Entry>() {
        entry.connect_changed(move |_| changed());
        entry.connect_activate(move |_| activate());
    } else if let Some(entry) = entry.downcast_ref::<gtk::PasswordEntry>() {
        entry.connect_changed(move |_| changed());
        entry.connect_activate(move |_| activate());
    }
}

/// What the dialog says, naming the network.
fn text(request: &Request) -> String {
    let network = glib::markup_escape_text(&request.network);
    let medium = match request.medium {
        Medium::Wifi => "Wi-Fi network",
        Medium::Wired => "wired network",
        Medium::Vpn => "VPN",
    };
    let secret = match &request.question {
        Question::Psk | Question::Sae => "password",
        Question::WepKey { passphrase: false } => "WEP key",
        Question::WepKey { passphrase: true } => "WEP passphrase",
        Question::Password { .. } => "username and password",
        Question::PrivateKeyPassphrase => "private key's passphrase",
        // The plugin's own words, as "You need to authenticate to access the Virtual
        // Private Network 'Work'.", where it has some.
        Question::Vpn { description, .. } if !description.is_empty() => {
            return glib::markup_escape_text(description).to_string();
        }
        Question::Vpn { .. } => "login",
    };
    if request.again {
        format!("The {secret} for the {medium} <b>{network}</b> didn't work. Type it again.")
    } else {
        format!("The {medium} <b>{network}</b> needs your {secret}.")
    }
}

fn placeholder(question: &Question) -> &'static str {
    match question {
        Question::Psk | Question::Sae | Question::Password { .. } => "Password",
        Question::WepKey { passphrase: false } => "Key",
        Question::WepKey { passphrase: true } | Question::PrivateKeyPassphrase => "Passphrase",
        // Each field has its label.
        Question::Vpn { .. } => "",
    }
}

fn label(markup: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_markup(markup);
    label.set_wrap(true);
    label.set_max_width_chars(44);
    label.set_xalign(0.0);
    label
}
