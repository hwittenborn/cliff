use std::{sync::Arc, thread, time::Duration};

use crate::util;
use futures::executor::BlockAwait;
use relm4::{
    adw::{self, prelude::*},
    gtk::{ScrolledWindow, TextBuffer, TextView},
    prelude::*,
};
use tokio::sync::{mpsc, Mutex};

/// Show an error screen.
pub fn show_error(primary_text: &str, secondary_text: Option<&str>) {
    let dialog = adw::MessageDialog::builder()
        .heading(primary_text)
        .body(secondary_text.unwrap_or(""))
        .build();
    dialog.add_response("", &tr::tr!("Ok"));
    dialog.show();
}

// Show an error screen with a codeblock.
pub fn show_codeblock_error(primary_text: &str, secondary_text: Option<&str>, code: &str) {
    let dialog = adw::MessageDialog::builder()
        .heading(primary_text)
        .body(secondary_text.unwrap_or(""))
        .extra_child(&codeblock(code))
        .resizable(true)
        .build();
    dialog.add_response("", &tr::tr!("Ok"));
    dialog.show();
}

/// Create a codeblock.
pub fn codeblock(text: &str) -> ScrolledWindow {
    let buffer = TextBuffer::builder().text(text).build();
    let block = TextView::builder()
        .buffer(&buffer)
        .editable(false)
        .focusable(false)
        .monospace(true)
        .build();
    ScrolledWindow::builder()
        .child(&block)
        .hexpand(true)
        .vexpand(true)
        .min_content_width(100)
        .min_content_height(100)
        .margin_top(10)
        .css_classes(vec![util::css::SCROLLABLE_CODEBLOCK.to_string()])
        .build()
}

/// A button that can be shown on an [`adw::EntryRow`].
///
/// The view returned is a [`gtk::Button`]. Any text/icons can be set by the
/// methods available on that.
#[relm4::widget_template(pub)]
impl WidgetTemplate for EntryRowButton {
    view! {
        #[name(button)]
        gtk::Button {
            add_css_class: "flat",
            set_valign: gtk::Align::Center
        }
    }
}

// /// A clipboard button that can be shown on an [`adw::EntryRow`]. It changes
// the /// clipboard icon to a checkmark after three seconds.
// ///
// /// `cb_text` is the text that should be copied upon clicking the clipboard
// /// button.
// pub fn entry_row_clipboard_button<T: ToString>(cb_text: T) -> EntryRowButton
// {     let cb_text = cb_text.to_string();

//     // Only change the icon back after all calls to the clipboard button have
// been     // sent.
//     //
//     // Without this, the following issue would be present:
//     // - Click the clipboard button
//     // - Click it again two seconds later
//     // - We *should* be waiting three seconds after the second click to go
// back to     //   the clipboard icon, but the first one would be closing it.
//     println!("GOT HERE");
//     let counter: Arc<Mutex<usize>> = Arc::default();
//     let (tx, mut rx) = mpsc::unbounded_channel::<()>();
//     println!("GOT HERE 1");

//     let label = gtk::Label::new(Some(&tr::tr!("Copied")));

//     relm4::view! {
//         #[name(widget)]
//         #[template]
//         EntryRowButton {
//             set_icon_name: crate::icons::COPY,

//             connect_clicked[counter, tx] => move |button| {
//                 button.set_icon_name(crate::icons::CHECKMARK);
//                 button.display().clipboard().set_text(&cb_text);

//                 // TODO: For some reason we get compiler errors about
// lifetimes without this. Need to find out why.                 println!("GOT
// HERE 2");                 let counter = counter.clone();
//                 let tx = tx.clone();
//                 println!("GOT HERE 3");

//                 glib::spawn_future_local(async move {
//                     println!("GOT HERE 4");
//                     *counter.lock().block_await() += 1;
//                     tokio::task::spawn_blocking(||
// thread::sleep(Duration::from_secs(3))).await.unwrap();                     
// println!("GOT HERE 5");                     tx.send(()).unwrap();
//                     println!("GOT HERE 6");
//                 });
//             }
//         }
//     }

//     println!("We here now");
//     let widget_clone = widget.clone();

//     println!("HERE?");
//     glib::spawn_future_local(async move {
//         println!("NOW HERE");
//         let widget = widget_clone;

//         println!("GOT HERE 7");
//         while let Some(_) = rx.recv().await {
//             println!("GOT HERE 8");
//             let mut lock = counter.lock().await;
//             *lock -= 1;

//             if *lock == 0 {
//                 widget.button.set_icon_name(crate::icons::COPY);
//             }
//         }
//     });

//     println!("All the way down here");
//     widget
// }
