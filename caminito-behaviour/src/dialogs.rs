//! The two dialogs the pages ask with: a choice among buttons, and a line of text.

use std::cell::RefCell;
use std::rc::Rc;

use futures_channel::oneshot;
use gtk::glib;
use gtk::prelude::*;

use crate::SPACING;

/// Asks `message` with `buttons`, the first of which cancels and the last of which is the
/// default, and returns the one chosen.
pub(crate) async fn choose(
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
pub(crate) async fn ask_text(
    window: Option<&gtk::Window>,
    title: &str,
    label: &str,
    initial: &str,
) -> Option<String> {
    let dialog = gtk::Window::builder()
        .modal(true)
        .resizable(false)
        .title(title)
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
        .spacing(SPACING / 3)
        .halign(gtk::Align::End)
        .build();
    buttons.append(&cancel);
    buttons.append(&ok);
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING / 3)
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
