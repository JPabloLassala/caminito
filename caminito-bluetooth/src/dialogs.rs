//! The pairing agent's dialogs (256), in GTK4: a small window for each question or code
//! BlueZ has, one at a time. The agent decides when one opens and closes; this draws them.
//!
//! The windows aren't the application's, so that opening the settings window raises that
//! and not a dialog (window.rs presents the application's first window).

use std::cell::RefCell;
use std::rc::Rc;

use caminito_bluetooth::agent::{
    Answer, Code, Dialogs, PASSKEY_MAX, PIN_CODE_LENGTH, Question, Reply,
};
use caminito_bluetooth::bluez::Device;
use gtk::glib;
use gtk::prelude::*;

/// The dialogs, drawn as GTK windows.
#[derive(Default)]
pub(crate) struct GtkDialogs {
    open: RefCell<Option<Open>>,
}

/// The dialog open.
struct Open {
    window: gtk::Window,
    /// The code it shows, when it shows one.
    code: Option<gtk::Label>,
    /// The device's path.
    device: String,
}

impl Dialogs for GtkDialogs {
    fn ask(&self, device: &Device, question: Question, reply: Reply) {
        self.close();
        let reply = Rc::new(RefCell::new(Some(reply)));
        let window = window();
        let content = content(&window);
        let name = glib::markup_escape_text(&device.alias);

        let (text, entry, yes) = match &question {
            Question::PinCode => (
                format!(
                    "Type the PIN code for <b>{name}</b>. Some devices print theirs, often 0000."
                ),
                Some(entry(PIN_CODE_LENGTH, false)),
                "Pair",
            ),
            Question::Passkey => (
                format!("Type the passkey that <b>{name}</b> shows."),
                Some(entry(6, true)),
                "Pair",
            ),
            Question::Confirmation { passkey } => (
                format!(
                    "Does <b>{name}</b> show this number?\n\n<span size=\"xx-large\">{passkey:06}</span>"
                ),
                None,
                "Pair",
            ),
            Question::Authorization => (
                format!("<b>{name}</b> wants to pair with this computer."),
                None,
                "Allow",
            ),
            Question::Service { uuid } => (
                format!(
                    "<b>{name}</b> wants to use a service of this computer.\n\n<small>{uuid}</small>"
                ),
                None,
                "Allow",
            ),
        };
        content.append(&label(&text));
        if let Some(entry) = &entry {
            content.append(entry);
        }

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.set_halign(gtk::Align::End);
        let no = gtk::Button::with_label("Cancel");
        let yes = gtk::Button::with_label(yes);
        yes.add_css_class("suggested-action");
        buttons.append(&no);
        buttons.append(&yes);
        content.append(&buttons);

        let answer = {
            let entry = entry.clone();
            let question = question.clone();
            move || match (&question, &entry) {
                (Question::PinCode, Some(entry)) => Answer::PinCode(entry.text().into()),
                (Question::Passkey, Some(entry)) => {
                    entry.text().parse().map_or(Answer::No, Answer::Passkey)
                }
                _ => Answer::Yes,
            }
        };
        if let Some(entry) = &entry {
            let sensitive = yes.clone();
            let valid = move |entry: &gtk::Entry| match &question {
                Question::Passkey => entry
                    .text()
                    .parse::<u32>()
                    .is_ok_and(|key| key <= PASSKEY_MAX),
                // BlueZ counts a PIN's bytes, not its characters.
                _ => (1..=PIN_CODE_LENGTH).contains(&entry.text().len()),
            };
            yes.set_sensitive(false);
            entry.connect_changed(move |entry| sensitive.set_sensitive(valid(entry)));
            entry.connect_activate({
                let yes = yes.clone();
                move |_| {
                    if yes.is_sensitive() {
                        yes.emit_clicked();
                    }
                }
            });
        }

        yes.connect_clicked({
            let reply = reply.clone();
            let window = window.clone();
            move |_| {
                send(&reply, answer());
                window.close();
            }
        });
        no.connect_clicked({
            let window = window.clone();
            move |_| window.close()
        });
        // Closing it any way the user can is a no. When the answer was sent already, or the
        // agent closed the dialog, there's no reply left or the agent ignores it.
        window.connect_close_request(move |_| {
            send(&reply, Answer::No);
            glib::Propagation::Proceed
        });

        window.present();
        *self.open.borrow_mut() = Some(Open {
            window,
            code: None,
            device: device.path.clone(),
        });
    }

    fn show(&self, device: &Device, code: Code) {
        let text = match &code {
            Code::PinCode(pin) => glib::markup_escape_text(pin).to_string(),
            Code::Passkey { passkey, entered } => {
                let digits = format!("{passkey:06}");
                let typed = usize::from(*entered).min(digits.len());
                format!("<u>{}</u>{}", &digits[..typed], &digits[typed..])
            }
        };
        let code_text = format!("<span size=\"xx-large\">{text}</span>");
        // The user may have closed the device's code: it shows again then.
        if let Some(Open {
            window,
            code: Some(code),
            device: path,
        }) = &*self.open.borrow()
            && *path == device.path
            && window.is_visible()
        {
            code.set_markup(&code_text);
            return;
        }

        self.close();
        let window = window();
        let content = content(&window);
        let name = glib::markup_escape_text(&device.alias);
        content.append(&label(&format!(
            "Type this code on <b>{name}</b>, then press Enter on it."
        )));
        let code = label(&code_text);
        code.set_halign(gtk::Align::Center);
        content.append(&code);
        let close = gtk::Button::with_label("Close");
        close.set_halign(gtk::Align::End);
        close.connect_clicked({
            let window = window.clone();
            move |_| window.close()
        });
        content.append(&close);
        window.present();
        *self.open.borrow_mut() = Some(Open {
            window,
            code: Some(code),
            device: device.path.clone(),
        });
    }

    fn close(&self) {
        let open = self.open.borrow_mut().take();
        if let Some(Open { window, .. }) = open
            && window.is_visible()
        {
            window.close();
        }
    }
}

/// Sends the dialog's one answer: the first of its buttons or its closing.
fn send(reply: &RefCell<Option<Reply>>, answer: Answer) {
    if let Some(reply) = reply.borrow_mut().take() {
        reply.send(answer);
    }
}

fn window() -> gtk::Window {
    gtk::Window::builder()
        .title("Bluetooth Pairing")
        .resizable(false)
        .default_width(360)
        .build()
}

/// The window's column of widgets.
fn content(window: &gtk::Window) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    window.set_child(Some(&content));
    content
}

fn label(markup: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_markup(markup);
    label.set_wrap(true);
    label.set_max_width_chars(40);
    label.set_xalign(0.0);
    label
}

fn entry(length: usize, digits: bool) -> gtk::Entry {
    let length = i32::try_from(length).expect("an entry's length is small");
    let entry = gtk::Entry::builder().max_length(length).build();
    if digits {
        entry.set_input_purpose(gtk::InputPurpose::Digits);
    }
    entry
}
