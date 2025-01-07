//! The data for a Google Drive Rclone config.
use super::ServerType;
use crate::{
    gtk_util,
    login::{dropbox, login_util, pcloud},
    mpsc::Sender,
    traits::prelude::*,
    util,
};
use adw::{glib, gtk::Button, gtk::Widget, prelude::*, ApplicationWindow, MessageDialog, SwitchRow};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use rocket::response::{content::RawHtml, Responder};
use std::{
    cell::RefCell,
    fmt,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    rc::Rc,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tera::{Context, Tera};

static DEFAULT_CLIENT_ID: &str =
    "617798216802-gpgajsc7o768ukbdegk5esa3jf6aekgj.apps.googleusercontent.com";
static DEFAULT_CLIENT_SECRET: &str = "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";

// The server type we're generating.
pub enum AuthType {
    Dropbox,
    GDrive,
    PCloud,
}

impl fmt::Display for AuthType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Dropbox => "Dropbox",
                Self::GDrive => "Google Drive",
                Self::PCloud => "pCloud",
            }
        )
    }
}

lazy_static::lazy_static! {
    // The state URL for the Rocket HTTP server to use in [`get_google_drive`] below.
    static ref STATE_URL: Mutex<String> = Mutex::new(String::new());
}

// Rocket routes for our Google Drive integration.
#[rocket::get("/")]
fn get_google_drive() -> RawHtml<String> {
    let mut context = Context::new();
    context.insert("state_url", STATE_URL.lock().unwrap().as_str());
    RawHtml(
        Tera::one_off(
            include_str!("../html/google-drive.tera.html"),
            &context,
            true,
        )
        .unwrap(),
    )
}

#[derive(Responder)]
#[response(content_type = "image/png")]
struct PngResponse(&'static [u8]);

#[rocket::get("/google-signin.png")]
fn get_google_signin_png() -> PngResponse {
    PngResponse(include_bytes!("../images/google-signin.png"))
}

#[rocket::get("/google-drive.png")]
fn get_google_drive_png() -> PngResponse {
    PngResponse(include_bytes!("../images/google-drive.png"))
}

#[derive(Clone, Debug, Default)]
pub struct GDriveConfig {
    pub server_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub auth_json: String,
}

impl super::LoginTrait for GDriveConfig {
    fn get_sections(
        window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
    ) -> (Vec<Widget>, Button) {
        Self::auth_sections(
            window,
            sender,
            AuthType::GDrive,
            DEFAULT_CLIENT_ID.to_owned(),
            DEFAULT_CLIENT_SECRET.to_owned(),
        )
    }
}

impl GDriveConfig {
    pub fn auth_sections(
        window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
        auth_type: AuthType,
        client_id: String,
        client_secret: String
    ) -> (Vec<Widget>, Button) {
        let mut sections: Vec<Widget> = vec![];
        let server_name = login_util::server_name_input();
        let in_europe = SwitchRow::builder().title(&tr::tr!("European server")).active(false).build();
        if matches!(auth_type, AuthType::PCloud) {
            sections.push(in_europe.clone().into());
        }
        let submit_button = login_util::submit_button();

        sections.push(server_name.clone().into());

        submit_button.connect_clicked(glib::clone!(@weak window, @weak server_name, @weak in_europe, @strong client_id, @strong client_secret => move |_| {
            window.set_sensitive(false);

            // For some reason we get compiler errors without these two lines :P.
            let client_id = client_id.clone();
            let client_secret = client_secret.clone();

            // Set up the rclone auth process.
            let mut args = vec!["authorize"];
            args.push(match auth_type {
                AuthType::GDrive => "drive",
                AuthType::Dropbox => "dropbox",
                AuthType::PCloud => "pcloud",
            });
            args.push(&client_id);
            args.push(&client_secret);
            if let AuthType::GDrive = auth_type {
                args.push("--auth-no-open-browser");
            }

            let mut process = Command::new("rclone")
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let process_stdout = Arc::new(Mutex::new(String::new()));
            let process_stderr = Arc::new(Mutex::new(String::new()));
            let stdout_handle = process.stdout.take().unwrap();
            let stderr_handle = process.stderr.take().unwrap();
            let _stdout_thread = thread::spawn(glib::clone!(@strong process_stdout => move || {
                let reader = BufReader::new(stdout_handle);
                for line in reader.lines() {
                    let string = line.unwrap();
                    process_stdout.lock().unwrap().push_str(&string);
                    process_stdout.lock().unwrap().push('\n');
                }
            }));
            let _stderr_thread = thread::spawn(glib::clone!(@strong process_stderr => move || {
                let reader = BufReader::new(stderr_handle);
                for line in reader.lines() {
                    let string = line.unwrap();
                    process_stderr.lock().unwrap().push_str(&string);
                    process_stderr.lock().unwrap().push('\n');
                }
            }));

            // Get the URL rclone will use for authentication by reading the process' stderr.
            loop {
                // If the rclone process has already aborted, go ahead and break so we can show the error down below.
                if process.try_wait().unwrap().is_some() {
                    break
                }

                // Otherwise check if the URL line has been printed, by checking for the auth URL.
                //
                // We check on both stdout and stderr, as rclone seems to report the authentication
                // URL differently depending on the version we're using.
                let process_output = format!(
                    "{}\n{}",
                    process_stdout.lock().unwrap(),
                    process_stderr.lock().unwrap(),
                );
                if let Some(line) = process_output.lines().find(|line| line.contains("http://127.0.0.1:53682/auth")) {
                 // The URL will be the last space-separated item on the line.
                    *STATE_URL.lock().unwrap() = line.split_whitespace().last().unwrap().to_owned();
                    break
                }

                util::run_in_background(|| thread::sleep(Duration::from_millis(500)));
            }

            hw_msg::warningln!("STATE URL: {}", STATE_URL.lock().unwrap());
            let kill_request = Rc::new(RefCell::new(false));

            // Set up and open the temporary HTTP server.
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let handle = runtime.spawn(rocket::build()
                .mount("/", rocket::routes![get_google_drive, get_google_signin_png, get_google_drive_png])
                .launch()
            );
            if let AuthType::GDrive = auth_type {
                Command::new("xdg-open").arg("http://localhost:8000").spawn().unwrap().wait().unwrap().exit_ok().unwrap();
            }

            // Wait for input from the user.
            let dialog = MessageDialog::builder()
                .heading(&tr::tr!("Authenticating to {}...", auth_type))
                .body(&tr::tr!("Follow the link that opened in your browser, and come back once you've finished."))
                .build();
            dialog.add_response("cancel", &tr::tr!("Cancel"));
            dialog.connect_response(None, glib::clone!(@strong kill_request => move |dialog, resp| {
                if resp != "cancel" {
                    return
                }

                dialog.close();
                *kill_request.get_mut_ref() = true;
            }));
            dialog.show();

            // Run until the process exits or the user clicks 'Cancel'.
            loop {
                // Sleep a little so the UI has a chance to process.
                util::run_in_background(|| thread::sleep(Duration::from_millis(500)));

                // Check if the user clicked cancel.
                if *kill_request.get_ref() {
                    handle.abort();
                    signal::kill(Pid::from_raw(process.id().try_into().unwrap()), Signal::SIGTERM).unwrap();
                    window.set_sensitive(true);
                    break;
                // Otherwise if the temporary webserver has died off, report it.
                } else if handle.is_finished() {
                    let error_string = util::await_future(handle).unwrap().unwrap_err().to_string();
                    gtk_util::show_codeblock_error(&tr::tr!("There was an issue while running the webserver for authentication"), &error_string);
                    window.set_sensitive(true);
                    break;
                // Otherwise if the command has finished, check if it returned a good exit code and then return it.
                } else if let Some(exit_status) = process.try_wait().unwrap() {
                    handle.abort();
                    dialog.close();

                    if !exit_status.success() {
                        gtk_util::show_codeblock_error(&tr::tr!("There was an issue authenticating to {}", auth_type), &process_stderr.lock().unwrap());
                        window.set_sensitive(true);
                        break;
                    } else {
                        let lines: Vec<String> = process_stdout.lock().unwrap().lines().map(|string| string.to_owned()).collect();
                        let auth_token = lines.get(lines.len() - 2).unwrap().to_owned();

                        let server_type = match auth_type {
                            AuthType::GDrive => ServerType::GDrive(GDriveConfig {
                                server_name: server_name.text().to_string(),
                                client_id,
                                client_secret,
                                auth_json: auth_token
                            }),
                            AuthType::Dropbox => ServerType::Dropbox(dropbox::DropboxConfig {
                                server_name: server_name.text().to_string(),
                                client_id,
                                client_secret,
                                auth_json: auth_token
                            }),
                            AuthType::PCloud => {
                                ServerType::PCloud(pcloud::PCloudConfig {
                                server_name: server_name.text().to_string(),
                                // hostname: ,
                                hostname: if in_europe.is_active() {"eapi.pcloud.com".to_string()} else {"api.pcloud.com".to_string()},
                                client_id,
                                client_secret,
                                auth_json: auth_token
                            })},
                        };
                        sender.send(Some(server_type));
                        window.set_sensitive(true);
                        break;
                    }
                }
            }
        }));
        server_name.connect_changed(glib::clone!(@weak submit_button => move |server_name| login_util::check_responses(&[server_name], &submit_button)));

        (sections, submit_button)
    }
}
