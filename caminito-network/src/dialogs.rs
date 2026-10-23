//! The secret agent's dialogs (ADR 0041), in GTK4: a small window for each secret
//! NetworkManager asks for, as many at once as it has requests. The agent decides when one
//! opens and closes; this draws them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use caminito_network::agent::{Answer, Dialogs, Medium, Question, Reply, Request};
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
        let window = gtk::Window::builder()
            .title("Network Authentication")
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

        // The identity, for an 802.1X password.
        let identity = match &request.question {
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
            .placeholder_text(placeholder(&request.question))
            .build();
        content.append(&secret);

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label("Cancel");
        let connect = gtk::Button::with_label("Connect");
        connect.add_css_class("suggested-action");
        connect.set_sensitive(false);
        buttons.append(&cancel);
        buttons.append(&connect);
        content.append(&buttons);

        let answer = {
            let identity = identity.clone();
            let secret = secret.clone();
            move || {
                let text = secret.text().to_string();
                match &identity {
                    Some(identity) => Answer::Login {
                        identity: identity.text().to_string(),
                        password: text,
                    },
                    None => Answer::Secret(text),
                }
            }
        };
        let validate = {
            let answer = answer.clone();
            let connect = connect.clone();
            let question = request.question.clone();
            move || connect.set_sensitive(question.accepts(&answer()))
        };
        secret.connect_changed({
            let validate = validate.clone();
            move |_| validate()
        });
        if let Some(identity) = &identity {
            identity.connect_changed(move |_| validate());
        }
        let activate = {
            let connect = connect.clone();
            move || {
                if connect.is_sensitive() {
                    connect.emit_clicked();
                }
            }
        };
        secret.connect_activate({
            let activate = activate.clone();
            move |_| activate()
        });
        if let Some(identity) = &identity {
            identity.connect_activate(move |_| activate());
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

        // An 802.1X identity that's there already only wants its password.
        match identity.filter(|identity| identity.text().is_empty()) {
            Some(identity) => identity.grab_focus(),
            None => secret.grab_focus(),
        };
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

/// What the dialog says, naming the network.
fn text(request: &Request) -> String {
    let network = glib::markup_escape_text(&request.network);
    let medium = match request.medium {
        Medium::Wifi => "Wi-Fi network",
        Medium::Wired => "wired network",
    };
    let secret = match &request.question {
        Question::Psk | Question::Sae => "password",
        Question::WepKey { passphrase: false } => "WEP key",
        Question::WepKey { passphrase: true } => "WEP passphrase",
        Question::Password { .. } => "username and password",
        Question::PrivateKeyPassphrase => "private key's passphrase",
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
