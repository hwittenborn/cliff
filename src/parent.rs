use crate::{gtk_util, util, SentryPanicInfo};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use relm4_components::alert::{Alert, AlertMsg, AlertSettings};
use std::{borrow::Cow, env, fs, path::Path, process::{Command, Stdio}};
use tempfile::NamedTempFile;

static SENTRY_DSN: &str =
    "https://e2b3dfc44a5b51cf2a8802130b9d1073@o4505099657740288.ingest.sentry.io/4506271301500928";

#[derive(Debug)]
enum ParentMsg {
    #[doc(hidden)]
    Close,
}

struct ParentModel {
    alert: Controller<Alert>,
}

/// The widgets to show and upload the backtrace.
fn panic_widgets(panic_info: SentryPanicInfo) -> gtk::ListBox {
    let backtrace = format!("{:?}", panic_info.backtrace);
    let event_id = panic_info.event.event_id;

    relm4::view! {
        #[name(panic_widgets)]
        gtk::ListBox {
            add_css_class: util::css::BOXED_LIST,
            set_margin_top: 5,
            set_selection_mode: gtk::SelectionMode::None,

            append = &adw::ExpanderRow {
                set_title: &tr::tr!("Crash logs"),
                #[template]
                add_suffix = &gtk_util::EntryRowButton {
                    set_icon_name: relm4_icons::icon_names::COPY,

                    connect_clicked[backtrace] => move |button| {
                        button.clipboard().set_text(&backtrace)
                    }
                },

                add_row = &gtk::ScrolledWindow {
                    set_height_request: 100,

                    #[wrap(Some)]
                    set_child = &gtk::TextView {
                        set_editable: false,
                        set_focusable: false,
                        set_monospace: true,

                        #[wrap(Some)]
                        set_buffer = &gtk::TextBuffer {
                            set_text: &backtrace
                        }
                    }
                }
            },

            #[name(crash_row)]
            append = &adw::ActionRow {
                set_title: &tr::tr!("Upload crash logs"),
                set_visible: util::APP_RELEASE_MODE,

                #[template]
                add_suffix = &gtk_util::EntryRowButton {
                    set_icon_name: relm4_icons::icon_names::COPY,

                    connect_clicked[crash_row] => move |button| {
                        button.clipboard().set_text(&crash_row.subtitle().unwrap())
                    }
                },

                #[template]
                add_suffix = &gtk_util::EntryRowButton {
                    set_icon_name: relm4_icons::icon_names::SHARE,

                    connect_clicked[crash_row] => move |button| {
                        // Show a loading animation while we upload logs.
                        let spinner = gtk::Spinner::new();
                        spinner.start();
                        button.set_child(Some(&spinner));

                        // Upload logs.
                        let guard = sentry::init((
                            SENTRY_DSN,
                            sentry::ClientOptions {
                                release: sentry::release_name!(),
                                default_integrations: false,
                                environment: util::APP_ENVIRONMENT.map(|env| Cow::Owned(env.to_owned())),
                                ..Default::default()
                            }
                        ));


                        // Attach some helpful info to the event, in the case that it gets uploaded.
                        let mut event = panic_info.event.clone(); // TODO: I got no clue why we need this, but we get compiler errors otherwise.
                        event.tags.insert("commit".to_owned(), crate::built::GIT_COMMIT_HASH.unwrap().to_owned());
                        event.tags.insert("arch".to_owned(), env::consts::ARCH.to_owned());
                        event.tags.insert("os".to_owned(), env::consts::OS.to_owned());

                        let uuid = sentry::capture_event(event).to_string();
                        let owned_button = button.to_owned();
                        let cloned_crash_row = crash_row.clone(); // TODO: I got no clue why we need this, but we get compiler errors otherwise.

                        relm4::spawn_local(async move {
                            relm4::spawn_blocking(move || {
                                // TODO: We need to not show a UUID when the event fails to send.
                                guard.flush(None);
                            })
                            .await
                            .unwrap();
                            cloned_crash_row.set_title(&tr::tr!("Support ID"));
                            cloned_crash_row.set_subtitle(&uuid);
                            cloned_crash_row.remove(&owned_button);
                        });
                    }
                }
            }
        }
    }

    return panic_widgets;
}

#[relm4::component]
impl SimpleComponent for ParentModel {
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

    fn init(
        panic_info: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut error_msg = tr::tr!("An unknown error has occured while running Celeste.");
        error_msg.push_str(&if util::APP_RELEASE_MODE {
            format!(
                "\n\n{}\n\n{}\n\n{}",
                tr::tr!("If you'd like to submit crash logs, you have the option to do so below. You can also view and copy them yourself, though before sharing them be aware that they may contain sensitive information."),
                tr::tr!("Submitting crash logs is optional but highly encouraged. Your crash logs will only be viewable by trusted Celeste staff members."),
                tr::tr!("If you do decide to submit crash logs, you'll get a support ID that you can use in Celeste's support rooms to aid in your request.")
            )
        } else {
            format!(" {}", tr::tr!("Crash logs can be viewed and copied below."))
        });

        let settings = AlertSettings {
            text: Some(tr::tr!("Unknown error")),
            secondary_text: Some(error_msg),
            confirm_label: Some("Ok".to_string()),
            extra_child: Some(panic_widgets(panic_info).into()),
            ..Default::default()
        };

        let alert = Alert::builder()
            .launch(settings)
            .connect_receiver(glib::clone!(@weak root => move |_, _| root.destroy()));
        alert.emit(AlertMsg::Show);

        let model = Self { alert };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}

fn handle_panic(path: &Path) {
    let content = fs::read_to_string(path).unwrap();
    let panic_info: SentryPanicInfo = serde_json::from_str(&content).unwrap();

    crate::create_app()
        .visible_on_activate(false)
        .run::<ParentModel>(panic_info);
}

pub fn start(background: bool) {
    // Create a temporary file to get the child's panic information from.
    let panic_file = NamedTempFile::new().unwrap();

    // Start up the Celeste subprocess, checking our panic file for errors if it
    // doesn't exit succesfully.
    let exe = env::current_exe().unwrap();
    let mut cmd_root = Command::new(exe);
    let mut cmd = cmd_root
        .env("PANIC_FILE", panic_file.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if background {
        cmd = cmd.arg("--background");
    }

    let output = cmd.spawn().unwrap().wait_with_output().unwrap();

    println!("STDOUT:\n{}\n===", std::str::from_utf8(&output.stdout).unwrap());
    println!("STDERR:\n{}\n===", std::str::from_utf8(&output.stderr).unwrap());
    if !output.status.success() {
        handle_panic(panic_file.path());
    }
}
