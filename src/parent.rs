use crate::{gtk_util, util, SentryPanicInfo};
use adw::prelude::*;
use relm4::prelude::*;
use relm4_components::alert::{Alert, AlertMsg, AlertSettings};
use std::{env, fs, path::Path, process::Command};
use tempfile::NamedTempFile;

#[derive(Debug)]
enum ParentMsg {
    #[doc(hidden)]
    Close,
}

struct ParentModel {
    alert: Controller<Alert>,
}

#[relm4::component(async)]
impl SimpleAsyncComponent for ParentModel {
    type Input = ParentMsg;
    type Output = ();
    type Init = SentryPanicInfo;

    // We don't want to show anything here since all of our logic is with the
    // [`Controller<Alert>`] component our struct contains, but we have to have
    // *something*, so include that here.
    view! {
        adw::ApplicationWindow {
        }
    }

    async fn init(
        panic_info: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let alert = Alert::builder()
            .transient_for(root.clone())
            .launch(AlertSettings {
                text: tr::tr!("Unknown error"),
                secondary_text: Some(format!(
                    "{}\n\n{} {}\n\n{}",
                    tr::tr!("An unknown error has occured while running Celeste."),
                    tr::tr!("If you'd like to submit crash logs, you hvae the option to do so below."),
                    tr::tr!("Submitting crash logs is optional but highly encouraged, as it helps Celeste's developerse find and resolve these issues."),
                    tr::tr!("If you do decide to submit crash logs, you'll get a support ID that you can use in Celeste's support rooms to help with anything related to this issue.")
                )),
                confirm_label: Some("Ok".to_string()),
                // widget: Some(gtk_util::codeblock("This is CODE !!").into()),
                ..Default::default()
            })
            .forward(sender.input_sender(), |_| ParentMsg::Close);
        alert.emit(AlertMsg::Show);
        let model = Self { alert };
        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }
}

fn handle_panic(path: &Path) {
    let content = fs::read_to_string(path).unwrap();
    let panic_info: SentryPanicInfo = serde_json::from_str(&content).unwrap();

    crate::create_app()
        .visible_on_activate(false)
        .run_async::<ParentModel>(panic_info);
}

pub fn start(background: bool) {
    // Create a temporary file to get the child's panic information from.
    let panic_file = NamedTempFile::new().unwrap();

    // Start up the Celeste subprocess, checking our panic file for errors if it
    // doesn't exit succesfully.
    let exe = env::current_exe().unwrap();
    let mut cmd_root = Command::new(exe);
    let mut cmd = cmd_root.env("PANIC_FILE", panic_file.path());

    if background {
        cmd = cmd.arg("--background");
    }

    let successful = cmd.spawn().unwrap().wait().unwrap().success();

    if !successful {
        handle_panic(panic_file.path());
    }
}

// fn panic() {
//     let bt = backtrace::Backtrace::new();
//     let bt_2 = std::backtrace::Backtrace::capture();
//     println!("BT:\n{bt:?}");
//     println!("BT 2:\n{bt_2}");
//     return;

//     // We need the runtime when we launch some functions in this code. Since
// the     // runtime that Relm4 usually sets up for us might not be alive
// anymore, we'll     // create our own and use it here.
//     let guard = sentry::init((
//         SENTRY_DSN,
//         ClientOptions {
//             release: sentry::release_name!(),
//             default_integrations: false,
//             ..Default::default()
//         },
//     ));

//     // do some hacky stuff to get it in there.
//     //
//     // Luckily [`Event`] can be serialized and deserialized, so use that to
// convert     // the event into a string and back out of one.
//     let backtrace = Backtrace::capture();
//     let event = PanicIntegration::new().event_from_panic_info(panic_info);

//     // TODO: Got to unwrap this after debugging.
//     let event_json = match serde_json::to_string(&event) {
//         Ok(event) => event,
//         Err(err) => {
//             println!("Didn't get an error: {err}");
//             "{}".to_string()
//     }};

//     // A panic message, formatted in the same way as [`std`]'s default
// handler.     let panic_msg = format!(
//         "thread '{}' {panic_info}\nstack backtrace:\n{backtrace}",
//         // TODO: Got to unwrap this after debugging.
//         match thread::current().name() {
//             Some(name) => name,
//             None => {
//                 println!("Didn't get the thread name !!");
//                 ""
//             }
//         }
//     );
//     eprintln!("{panic_msg}");

//     let app = adw::Application::default();
//     // For some reason [`adw::Application::set_application`] wants a
//     // [`gtk::Application`].
//     let gtk_app: gtk::Application = app.clone().into();

//     println!("WOW 1");
//     let context = glib::MainContext::new();
//     println!("WOW 2");
//     println!("Can we acquire? {:?}", context.acquire().is_ok());
//     context.spawn_local(async { println!("WOW 3") });
//     println!("WOW 4");
//     context.iteration(false);
//     println!("GOT TO WOW");

//     app.connect_activate(move |app| {
//         relm4::view! {
//             adw::MessageDialog {
//                 show: (),
//                 set_application: Some(&gtk_app),
//                 set_resizable: true,
//                 set_width_request: 400,
//                 set_heading: Some(&tr::tr!("Unknown error")),
//                 set_body: &format!(
//                     "{}\n\n{} {}\n\n{}",
//                     tr::tr!("An unknown error has occurred while running
// Celeste."),                     tr::tr!("If you'd like to submit crash logs,
// you have the option to do so below."),
// tr::tr!("Submitting crash logs is optional but highly encouraged, as it helps
// Celeste's developers find and resolve these issues."),
// tr::tr!("If you do decide to submit crash logs, you'll get a support ID that
// you can use in Celeste's support rooms to help with anything related to this
// issue.")                 ),
//                 add_response: ("", &tr::tr!("Ok")),

//                 #[wrap(Some)]
//                 set_extra_child = &gtk::ListBox {
//                     add_css_class: util::css::BOXED_LIST,
//                     set_margin_top: 5,
//                     set_selection_mode: gtk::SelectionMode::None,

//                     append = &adw::ExpanderRow {
//                         set_title: &tr::tr!("Crash logs"),
//                         // add_suffix:
// &*gtk_util::entry_row_clipboard_button(&panic_msg),

//                         add_row = &gtk::ScrolledWindow {
//                             set_height_request: 100,

//                             #[wrap(Some)]
//                             set_child = &gtk::TextView {
//                                 set_editable: false,
//                                 set_focusable: false,
//                                 set_monospace: true,

//                                 #[wrap(Some)]
//                                 set_buffer = &gtk::TextBuffer {
//                                     set_text: &panic_msg
//                                 }
//                             }
//                         }
//                     },

//                     #[name(crash_row)]
//                     append = &adw::ActionRow {
//                         set_title: &tr::tr!("Upload crash logs"),

//                         #[template]
//                         add_suffix = &gtk_util::EntryRowButton {
//                             set_icon_name: icons::SHARE,

//                             connect_clicked[event_json, crash_row] => move
// |button| {                                 println!("We're finally doing
// something in here");                                 // Get an owned
// [`Button`] so we can pass it into [`glib::spawn_future_local`] below.
//                                 let button = button.to_owned();

//                                 // TODO: For some reason we get a compiler
// error without these. We need to figure out why, because this is kind of
// hacky.                                 let crash_row = crash_row.clone();
//                                 let event_json = event_json.clone();

//                                 // Set up the button to load until we submit
// the crash log.                                 let spinner =
// gtk::Spinner::new();                                 spinner.start();
//                                 button.set_child(Some(&spinner));

//                                 // Process the event, and set the ID in the
// GUI.                                 println!("GLIB: {}",
// std::thread::current().name().unwrap());
// glib::spawn_future_local(async move {                                     //
// println!("Here now");                                     // let event: Event
// = serde_json::from_str(&event_json).unwrap();
// // let uuid = sentry::capture_event(event).to_string();

//                                     // println!("Something here");
//                                     // // relm4::spawn_blocking(||
// sentry::Hub::current().client().unwrap().flush(None)).await.unwrap();
//                                     // println!("Another something here");

//                                     // crash_row.set_title(&tr::tr!("Support
// ID"));                                     // crash_row.set_subtitle(&uuid);
//                                     // crash_row.remove(&button);
//                                     // println!("HERE");
//                                     // //
// crash_row.add_suffix(&*gtk_util::entry_row_clipboard_button(uuid));
//                                     // println!("ANOTHER HERE");
//                                 });
//                                 println!("GLIB NO 2");
//                             }
//                         },
//                     }
//                 }
//             }
//         }
//     });

//     println!("RT HERE");
//     let rt = Runtime::new().unwrap();
//     let handle = rt.handle();
//         app.run();
//     // handle.block_on(async {
//     //     println!("Local set testing");
//     //     tokio::time::sleep(Duration::from_secs(2)).await;
//     //     println!("We got through time!");
//     //     println!("In app");
//     // });

//     // Since we have the crate set to 'abort' on panics, we seem to get an
// 'Aborted'     // message in the terminal. I don't like that, so just exit the
// process.     // TODO: Use some kind of library for this code.
//     process::exit(1);
// }
