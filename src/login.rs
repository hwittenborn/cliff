//! Functionality for logging into a server.
use crate::{
    gtk_util,
    rclone::{self, RcloneConfigItem, WebDavVendors},
    traits::*,
    util,
};
use adw::{prelude::*, Application, ApplicationWindow};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use regex::Regex;
use relm4::{
    component::{AsyncComponentParts, AsyncComponentSender, SimpleAsyncComponent},
    gtk::{gdk, glib},
    once_cell::sync::Lazy,
    prelude::*,
};
use relm4_components::alert::{Alert, AlertMsg, AlertResponse, AlertSettings};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::json;
use std::{
    cell::RefCell,
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::SocketAddr,
    path::PathBuf,
    process::{Child, Command, Stdio},
    rc::Rc,
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use strum::{EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use tempfile::NamedTempFile;
use tera::{Context, Tera};
use tokio::sync::mpsc::{self, Receiver, Sender};
use url::Url;
use warp::{
    http::{header, Response},
    Filter,
};

/// The rclone template used after authentication is done.
static RCLONE_TEMPLATE: &str = include_str!("html/auth-template.go.html");

/// The Dropbox client ID/secret.
static DROPBOX_CLIENT: (&str, &str) = ("hke0fgr43viaq03", "o4cpx8trcnneq7a");

/// The Google Drive client ID/secret.
static GOOGLE_DRIVE_CLIENT: (&str, &str) = (
    "617798216802-gpgajsc7o768ukbdegk5esa3jf6aekgj.apps.googleusercontent.com",
    "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",
);

/// The Google SVG.
static GOOGLE_SVG: &[u8] = include_bytes!("icons/google.svg");

/// The pCloud client ID/secret.
static PCLOUD_CLIENT: (&str, &str) = ("KRzpo46NKb7", "g10qvqgWR85lSvEQWlIqCmPYIhwX");

/// Get an authentication token for the given provider. Returns [`Err`] if
/// unable to.
///
/// `rx` is an [`mpsc::Receiver`] to cancel authentication requests with.
async fn get_token(
    provider: Provider,
    mut rx: mpsc::Receiver<()>,
) -> Result<String, LoginCommandErr> {
    // Set up the rclone template file for usage later.
    let mut template_file = NamedTempFile::new().unwrap();
    template_file.write(RCLONE_TEMPLATE.as_bytes()).unwrap();

    let (client_id, client_secret) = match provider {
        Provider::Dropbox => DROPBOX_CLIENT,
        Provider::GoogleDrive => GOOGLE_DRIVE_CLIENT,
        Provider::PCloud => PCLOUD_CLIENT,
        _ => panic!("An invalid provider was entered"),
    };

    let rclone_args = [
        "authorize",
        provider.rclone_type(),
        client_id,
        client_secret,
        "--auth-no-open-browser",
        "--template",
        &template_file.path().display().to_string(),
    ];

    // Spawn the authentication process, and continuously read it's stdout and stdin
    // into strings.
    let rclone_stdout: Arc<Mutex<String>> = Arc::default();
    let rclone_stderr: Arc<Mutex<String>> = Arc::default();

    let mut process = Command::new("rclone")
        .args(&rclone_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let process_stdout = process.stdout.take().unwrap();
    let process_stderr = process.stderr.take().unwrap();
    let stdout_thread = thread::spawn(glib::clone!(@strong rclone_stdout => move || {
        let reader = BufReader::new(process_stdout);
        for line in reader.lines() {
            let mut string = line.unwrap();
            string.push('\n');
            rclone_stdout.lock().unwrap().push_str(&string);
        }
    }));
    let stderr_thread = thread::spawn(glib::clone!(@strong rclone_stderr => move || {
        let reader = BufReader::new(process_stderr);
        for line in reader.lines() {
            let mut string = line.unwrap();
            string.push('\n');
            rclone_stderr.lock().unwrap().push_str(&string);
        }
    }));

    // Get the URL rclone will use for authentication.
    let rclone_url = loop {
        // If the rclone process has aborted already, then it failed before being able
        // to get us a URL and we need to let the user know.
        if process.try_wait().unwrap().is_some() {
            break Err(rclone_stderr.lock().unwrap().to_string());
        }

        // Otherwise check if the URL line has been printed in stdout or stderr.
        // Currently in rclone, this involves checking for a URL at the end of a line.
        let output = format!(
            "{}\n{}",
            rclone_stdout.lock().unwrap(),
            rclone_stderr.lock().unwrap()
        );
        let maybe_url = output
            .lines()
            .find(|line| line.contains("http://127.0.0.1:53682/auth"))
            .map(|line| line.split_whitespace().last().unwrap().to_owned());

        if let Some(url) = maybe_url {
            break Ok(url);
        }
    }
    .map_err(|err| LoginCommandErr::AuthServer(err))?;

    // Present the authentication request to the user.
    let addr = rclone_url.to_string();
    open::that(&addr).unwrap();

    // Get the token, returning an error if we couldn't get it.
    let token = relm4::spawn_blocking(move || loop {
        // Check if the user is cancelling the request.
        if rx.try_recv().is_ok() {
            // Kill the rclone process so we can use it in subsequent requests.
            let pid = Pid::from_raw(process.id().try_into().unwrap());
            signal::kill(pid, Signal::SIGINT).unwrap();
            break Err(LoginCommandErr::Cancelled);
        // Otherwise if the command finished, check if it returned a good exit
        // code and then return the token.
        } else if let Some(exit_status) = process.try_wait().unwrap() {
            if !exit_status.success() {
                break Err(LoginCommandErr::Token(
                    rclone_stderr.lock().unwrap().to_string(),
                ));
            } else {
                let token = rclone_stdout
                    .lock()
                    .unwrap()
                    .lines()
                    .rev()
                    .nth(1)
                    .unwrap()
                    .to_owned();
                break Ok(token);
            }
        }
    })
    .await
    .unwrap();

    Ok(token?)
}

#[relm4::widget_template(pub)]
impl WidgetTemplate for WarningButton {
    view! {
        gtk::Button {
            add_css_class: "flat",
            set_icon_name: relm4_icons::icon_names::WARNING,
            set_valign: gtk::Align::Center
        }
    }
}

#[derive(Clone, Debug, Default, EnumIter, EnumString, IntoStaticStr, PartialEq, strum::Display)]
pub enum Provider {
    #[default]
    Dropbox,
    #[strum(serialize = "Google Drive")]
    GoogleDrive,
    Nextcloud,
    Owncloud,
    #[strum(serialize = "pCloud")]
    PCloud,
    #[strum(serialize = "Proton Drive")]
    ProtonDrive,
    WebDav,
}

impl Provider {
    /// The name rclone uses to identity this remote type.
    fn rclone_type(&self) -> &'static str {
        match self {
            Self::Dropbox => "dropbox",
            Self::GoogleDrive => "drive",
            Self::Nextcloud | Self::Owncloud | Self::WebDav => "webdav",
            Self::PCloud => "pcloud",
            Self::ProtonDrive => "protondrive",
        }
    }

    /// See if a provider uses the rclone WebDav backend.
    fn is_webdav(&self) -> bool {
        matches!(self, Self::Nextcloud | Self::Owncloud | Self::WebDav)
    }
}

#[derive(Clone, Debug)]
pub enum LoginMsg {
    /// Open the login window.
    Open,
    /// Set the provider we want to log in with.
    #[doc(hidden)]
    SetProvider(Provider),
    /// Check that the inputs provided by the user are valid.
    #[doc(hidden)]
    CheckInputs,
    /// Show an error from clicking the warning button on a login field.
    #[doc(hidden)]
    ShowFieldError(LoginField),
    /// Prepare for authentication, by showing a confirm authentication window
    /// as needed.
    ///
    /// This is done to conform to some service's auth requirements.
    #[doc(hidden)]
    PrepAuthenticate,
    /// Get a token for a service that needs it.
    #[doc(hidden)]
    Authenticate,
    /// Cancel an active authentication session from [`Self::Authenticate`].
    #[doc(hidden)]
    CancelAuthenticate,
}

#[derive(Debug)]
pub enum LoginOutput {
    // The remote name in the rclone config for the new login.
    NewLogin(String),
    // The user closed the window without logging in.
    LoginCancel,
}

/// The login fields that we need to check. We use this in [`LoginModel`] below.
#[derive(Clone, Debug, EnumIter, Eq, Hash, PartialEq)]
pub enum LoginField {
    Name,
    Url,
    Username,
    Password,
    Totp,
}

#[derive(Debug)]
pub enum LoginCommandErr {
    /// The authentication request was cancelled.
    Cancelled,
    /// An error from starting up the rclone authorization server. Contains the
    /// stderr of `rclone authorize`.
    AuthServer(String),
    /// An error from obtaining a token from the rclone authorization server.
    /// Contains the stderr of `rclone authorize`.
    Token(String),
    /// There was an error connecting to the remote server.
    NameResolution,
    /// The user entered an invalid password.
    InvalidPassword,
    /// A 2FA code was required, but not provided.
    MissingTotp,
    /// An unknown error was found while checking validity of credentials. The
    /// error contains the message from [`librclone`].
    ValidityUnknown(String),
}

/// The type we use to store errors. The values are a tuple of (title, subtitle)
/// messages to pass to a message window.
type Errors = HashMap<LoginField, (String, String)>;

pub struct LoginModel {
    visible: bool,
    provider: Provider,
    errors: Rc<RefCell<Errors>>,
    /// Whether the login spinner should be shown. This should be true when
    /// we're processing a login request.
    show_login_spinner: bool,
    /// An [`Sender`] to use when cancelling authentication requests from
    /// [`Self::auth`]. It gets set to [`Some`] at the start of an
    /// authentication request from [`LoginMsg::Authenticate`].
    auth_sender: Option<Sender<()>>,
    /// The [`Alert`] component we use for showing errors from
    /// [`Self::errors`].
    alert: Controller<Alert>,
    /// The [`Alert`] component we use before using [`Self::auth`]. Need to
    /// satisfy auth requirements of some cloud providers.
    prep_auth: Controller<Alert>,
    /// The [`Alert`] component we use to notify the user of web browser
    /// authentication.
    auth: Controller<Alert>,
}

#[relm4::component(async, pub)]
impl AsyncComponent for LoginModel {
    type Input = LoginMsg;
    type Output = LoginOutput;
    type CommandOutput = Result<String, LoginCommandErr>;
    type Init = ();

    view! {
        #[name(window)]
        ApplicationWindow {
            set_title: Some(&util::get_title!("Log in")),
            set_default_width: 400,
            add_css_class: "celeste-global-padding",
            #[watch]
            set_visible: model.visible,
            // When hiding/showing different entry widgets, we may end up with
            // extra padding on the bottom of the window. This resets the
            // window height to our widget size on each render.
            #[watch]
            set_default_size: (window.width(), -1),

             gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar,

                gtk::ListBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    add_css_class: "boxed-list",

                    adw::ComboRow {
                        set_title: &tr::tr!("Server Type"),

                        #[wrap(Some)]
                        set_model = &gtk::StringList {
                            #[iterate]
                            append: Provider::iter().map(|provider| provider.into())
                        },

                        connect_selected_item_notify[sender] => move |row| {
                            let string_list: gtk::StringList = row.model().unwrap().downcast().unwrap();
                            let selected = string_list.string(row.selected()).unwrap().to_string();
                            let provider = Provider::from_str(&selected).unwrap();
                            sender.input(LoginMsg::SetProvider(provider));
                        }
                    },

                    #[name(name_input)]
                    adw::EntryRow {
                        set_title: &tr::tr!("Name"),
                        #[template]
                        add_suffix = &WarningButton {
                            #[watch]
                            set_visible: !model.errors.borrow().get(&LoginField::Name).unwrap().0.is_empty(),
                            connect_clicked => LoginMsg::ShowFieldError(LoginField::Name)
                        },

                        connect_changed[errors = model.errors.clone(), sender] => move |name_input| {
                            let name = name_input.text().to_string();

                            // Get a list of already existing config names.
                            let existing_remotes: Vec<String> = rclone::get_remotes()
                                .block_await()
                                .iter()
                                .map(|config| config.remote_name())
                                .collect();

                            // Check that the new specified remote is valid.
                            static NAME_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[0-9a-zA-Z_.][0-9a-zA-Z_. -]*[0-9a-zA-Z_.-]$").unwrap());

                            let mut err_msg = None;

                            if name.is_empty() {
                                err_msg = None;
                            } else if existing_remotes.contains(&name) {
                                err_msg = (tr::tr!("Name already exists"), String::new()).into();
                            } else if !NAME_REGEX.is_match(&name) {
                                err_msg = (
                                    tr::tr!("Invalid server name"),
                                    format!(
                                        "{}\n- {}\n- {}\n- {}",
                                        tr::tr!("Server names must:"),
                                        tr::tr!("Only contain numbers, letters, underscores, hyphens, periods, and spaces"),
                                        tr::tr!("Not start with a hyphen/space"),
                                        tr::tr!("Not end with a space")
                                    )
                                ).into();
                            }

                            if let Some(msg) = err_msg {
                                *errors.borrow_mut().get_mut(&LoginField::Name).unwrap() = msg;
                                name_input.add_css_class(util::css::ERROR);
                            } else {
                                let mut borrow = errors.borrow_mut();
                                let items = borrow.get_mut(&LoginField::Name).unwrap();
                                items.0.clear();
                                items.1.clear();
                                name_input.remove_css_class(util::css::ERROR);
                            }

                            sender.input(LoginMsg::CheckInputs)
                        },
                    },

                    #[name(url_input)]
                    adw::EntryRow {
                        set_title: &tr::tr!("Server URL"),
                        #[watch]
                        set_visible: matches!(model.provider, Provider::Nextcloud | Provider::Owncloud | Provider::WebDav),
                        #[template]
                        add_suffix = &WarningButton {
                            #[watch]
                            set_visible: !model.errors.borrow().get(&LoginField::Url).unwrap().0.is_empty(),
                            connect_clicked => LoginMsg::ShowFieldError(LoginField::Url)
                        },
                        connect_changed[errors = model.errors.clone(), provider = model.provider.clone(), sender] => move |url_input| {
                            let mut err_msg = None;
                            let maybe_url = Url::parse(&url_input.text());

                            if url_input.text().is_empty() {
                                err_msg = None;
                            } else if let Ok(url) = maybe_url {
                                if matches!(provider, Provider::Nextcloud | Provider::Owncloud) && url.path().contains("/remote.php/") {
                                    static REMOTE_PHP_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"/remote\.php/.*").unwrap());
                                    let invalid_url_segment = REMOTE_PHP_REGEX.find(url.path())
                                        .unwrap()
                                        .as_str()
                                        .to_string();
                                    err_msg = (
                                        tr::tr!("Invalid server URL"),
                                        tr::tr!("Don't specify '{invalid_url_segment}' as part of the URL"),
                                    ).into();
                                }
                            } else {
                                err_msg = (
                                    tr::tr!("Invalid server URL"),
                                    tr::tr!("Error: {}.", maybe_url.unwrap_err())
                                ).into();
                            }

                            if let Some(msg) = err_msg {
                                *errors.borrow_mut().get_mut(&LoginField::Url).unwrap() = msg;
                                url_input.add_css_class(util::css::ERROR);
                            } else {
                                let mut borrow = errors.borrow_mut();
                                let items = borrow.get_mut(&LoginField::Url).unwrap();
                                items.0.clear();
                                items.1.clear();
                                url_input.remove_css_class(util::css::ERROR);
                            }

                            sender.input(LoginMsg::CheckInputs)
                        }
                    },

                    #[name(username_input)]
                    adw::EntryRow {
                        set_title: &tr::tr!("Username"),
                        #[watch]
                        set_visible: matches!(model.provider, Provider::Nextcloud | Provider::Owncloud | Provider::ProtonDrive | Provider::WebDav),
                        connect_changed => LoginMsg::CheckInputs,
                    },

                    #[name(password_input)]
                    adw::PasswordEntryRow {
                        set_title: &tr::tr!("Password"),
                        #[watch]
                        set_visible: matches!(model.provider, Provider::Nextcloud | Provider::Owncloud | Provider::ProtonDrive | Provider::WebDav),
                        connect_changed => LoginMsg::CheckInputs,
                    },

                    #[name(totp_input)]
                    adw::EntryRow {
                        set_title: &tr::tr!("2FA Code"),
                        set_editable: false,
                        #[watch]
                        set_visible: matches!(model.provider, Provider::ProtonDrive),
                        #[template]
                        add_suffix = &WarningButton {
                            #[watch]
                            set_visible: totp_input_checkmark.is_active() && !model.errors.borrow().get(&LoginField::Totp).unwrap().0.is_empty(),
                            connect_clicked => LoginMsg::ShowFieldError(LoginField::Totp)
                        },
                        connect_changed[errors = model.errors.clone(), sender] => move |totp_input| {
                            let totp = totp_input.text().to_string();
                            let mut err_msg = None;

                            if totp.is_empty() {
                                err_msg = None;
                            } else if totp.chars().any(|c| !c.is_numeric()) {
                                err_msg = (
                                    tr::tr!("Invalid 2FA code"),
                                    tr::tr!("The 2FA code should only contain digits")
                                ).into();
                            } else if totp.len() != 6 {
                                err_msg = (
                                    tr::tr!("Invalid 2FA code"),
                                    tr::tr!("The 2FA code should be 6 digits long")
                                ).into();
                            }

                            if let Some(msg) = err_msg {
                                *errors.borrow_mut().get_mut(&LoginField::Totp).unwrap() = msg;
                                totp_input.add_css_class(util::css::ERROR);
                            } else {
                                let mut borrow = errors.borrow_mut();
                                let items = borrow.get_mut(&LoginField::Totp).unwrap();
                                items.0.clear();
                                items.1.clear();
                                totp_input.remove_css_class(util::css::ERROR);
                            }

                            sender.input(LoginMsg::CheckInputs)
                        },

                        #[name(totp_input_checkmark)]
                        add_prefix = &gtk::CheckButton {
                            connect_toggled[sender, totp_input] => move |check| {
                                let active = check.is_active();
                                totp_input.set_editable(active);

                                if !active {
                                    totp_input.set_text("");
                                    totp_input.remove_css_class(util::css::ERROR);
                                }
                                sender.input(LoginMsg::CheckInputs);
                            }
                        }
                    }
                },

                gtk::Box {
                    set_halign: gtk::Align::Fill,
                    set_margin_top: 15,


                    gtk::Spinner {
                        start: (),
                        set_valign: gtk::Align::End,
                        #[watch]
                        set_visible: model.show_login_spinner
                    },

                    #[name(login_button)]
                    gtk::Button {
                        set_label: &tr::tr!("Log in"),
                        add_css_class: "login-window-submit-button",
                        set_sensitive: false,
                        set_halign: gtk::Align::End,
                        set_hexpand: true,
                        connect_clicked => LoginMsg::PrepAuthenticate,
                    }
                }
             }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // TODO: Use this to show Google Auth button.
        relm4::view! {
            #[name(prep_auth_button)]
            gtk::Button {
                set_halign: gtk::Align::Center,
                set_margin_top: 10,
                connect_clicked => LoginMsg::Authenticate,

                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_spacing: 10,

                    gtk::Image {
                        set_from_paintable: Some(&gdk::Texture::from_bytes(&glib::Bytes::from_static(GOOGLE_SVG)).unwrap())
                    },

                    gtk::Label {
                        set_label: &tr::tr!("Sign in with Google")
                    }
                }
            }
        }

        let alert = Alert::builder()
            .transient_for(root.clone())
            .launch(AlertSettings {
                confirm_label: Some(tr::tr!("Ok")),
                ..Default::default()
            })
            .connect_receiver(|_, _| {});
        let prep_auth = Alert::builder()
            .transient_for(root.clone())
            .launch(AlertSettings {
                text: Some(tr::tr!("Authenticate to Google Drive")),
                secondary_text: Some(tr::tr!("To confirm with Google that you want to connect to Google Drive, click the button below")),
                extra_child: Some(prep_auth_button.into()),
                ..Default::default()
            })
            .connect_receiver(|_, _| {});
        let auth = Alert::builder()
            .transient_for(root.clone())
            .launch(AlertSettings {
                text: None,
                secondary_text: Some(tr::tr!("Follow the link that opened in your browser, and come back once you've finished")),
                cancel_label: Some(tr::tr!("Cancel")),
                ..Default::default()
            })
            .forward(sender.input_sender(), |_| LoginMsg::CancelAuthenticate);

        let mut model = Self {
            visible: false,
            provider: Provider::default(),
            errors: Rc::default(),
            show_login_spinner: false,
            auth_sender: None,
            alert,
            prep_auth,
            auth,
        };
        for field in LoginField::iter() {
            model.errors.borrow_mut().insert(field, Default::default());
        }

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        // Reset all the input widgets we use to be empty. We do this when
        // opening/re-opening the window or switching providers.
        let reset_widgets = || {
            widgets.name_input.set_text("");
            widgets.url_input.set_text("");
            widgets.username_input.set_text("");
            widgets.password_input.set_text("");
            widgets.totp_input_checkmark.set_active(false);
        };

        match message {
            LoginMsg::Open => {
                reset_widgets();
                sender.input(LoginMsg::SetProvider(Provider::default()));
                self.visible = true;
            }
            LoginMsg::SetProvider(provider) => {
                reset_widgets();
                self.provider = provider;
            }
            LoginMsg::CheckInputs => {
                // Disable the login button if any current input fields are empty or contain
                // errors.
                let mut sensitive = true;
                let inputs: Vec<adw::EntryRow> = vec![
                    widgets.name_input.clone(),
                    widgets.url_input.clone(),
                    widgets.username_input.clone(),
                    widgets.password_input.clone().into(),
                ];

                for input in inputs {
                    if input.is_visible() {
                        if input.text().is_empty() || input.has_css_class(util::css::ERROR) {
                            sensitive = false;
                        }
                    }
                }

                // We have to check the TOTP field separately, as it contains a checkmark
                // toggle.
                if widgets.totp_input.is_visible() && widgets.totp_input_checkmark.is_active() {
                    if widgets.totp_input.text().is_empty()
                        || widgets.totp_input.has_css_class(util::css::ERROR)
                    {
                        sensitive = false;
                    }
                }

                widgets.login_button.set_sensitive(sensitive);
            }
            LoginMsg::ShowFieldError(field) => {
                let mut errors_ref = self.errors.borrow_mut();
                let error_items = errors_ref.get_mut(&field).unwrap();

                let mut alert_state = self.alert.state().get_mut();
                let mut settings = &mut alert_state.model.settings;

                settings.text = Some(error_items.0.clone());
                settings.secondary_text = Some(error_items.1.clone());
                self.alert.emit(AlertMsg::Show);
            }
            LoginMsg::PrepAuthenticate => {
                if self.provider == Provider::GoogleDrive {
                    self.prep_auth.emit(AlertMsg::Show);
                } else {
                    sender.input(LoginMsg::Authenticate);
                }
            }
            LoginMsg::Authenticate => {
                root.set_sensitive(false);
                self.show_login_spinner = true;

                let needs_auth_window = matches!(
                    self.provider,
                    Provider::Dropbox | Provider::GoogleDrive | Provider::PCloud
                );
                let (tx, rx) = mpsc::channel(1);

                if needs_auth_window {
                    self.auth.state().get_mut().model.settings.text =
                        Some(tr::tr!("Logging into {}...", self.provider));
                    self.auth.emit(AlertMsg::Show);
                }

                let provider = self.provider.clone();
                let mut widget_values = HashMap::from([
                    (LoginField::Name, widgets.name_input.text().to_string()),
                    (
                        LoginField::Username,
                        widgets.username_input.text().to_string(),
                    ),
                    (
                        LoginField::Password,
                        widgets.password_input.text().to_string(),
                    ),
                    (LoginField::Totp, widgets.totp_input.text().to_string()),
                    (LoginField::Url, {
                        // Nextcloud/Owncloud URLs needs the WebDav URL appended to them since we
                        // disallow it it in the UI.
                        let url = widgets.url_input.text().to_string();

                        if matches!(provider, Provider::Nextcloud | Provider::Owncloud) {
                            format!(
                                "{url}/remote.php/dav/files/{}",
                                widgets.username_input.text()
                            )
                        } else {
                            url
                        }
                    }),
                ]);
                let config_name = widgets.name_input.text().to_string();

                sender.oneshot_command(async move {
                    // Get a token if needed.
                    let token = if needs_auth_window {
                        Some(get_token(provider.clone(), rx).await?)
                    } else {
                        None
                    };

                    // Create the config entry.
                    rclone::sync::create_config(
                        &config_name,
                        match provider {
                            Provider::Dropbox => RcloneConfigItem::Dropbox {
                                client_id: DROPBOX_CLIENT.0.to_string(),
                                client_secret: DROPBOX_CLIENT.1.to_string(),
                                token: token.unwrap(),
                            },
                            Provider::GoogleDrive => RcloneConfigItem::GoogleDrive {
                                client_id: GOOGLE_DRIVE_CLIENT.0.to_string(),
                                client_secret: GOOGLE_DRIVE_CLIENT.1.to_string(),
                                token: token.unwrap(),
                            },
                            Provider::Nextcloud => RcloneConfigItem::WebDav {
                                url: widget_values.remove(&LoginField::Url).unwrap(),
                                vendor: WebDavVendors::Nextcloud,
                                user: widget_values.remove(&LoginField::Username).unwrap(),
                                pass: widget_values.remove(&LoginField::Password).unwrap(),
                            },
                            Provider::Owncloud => RcloneConfigItem::WebDav {
                                url: widget_values.remove(&LoginField::Url).unwrap(),
                                vendor: WebDavVendors::Owncloud,
                                user: widget_values.remove(&LoginField::Username).unwrap(),
                                pass: widget_values.remove(&LoginField::Password).unwrap(),
                            },
                            Provider::PCloud => RcloneConfigItem::PCloud {
                                client_id: PCLOUD_CLIENT.0.to_string(),
                                client_secret: PCLOUD_CLIENT.1.to_string(),
                                token: token.unwrap(),
                            },
                            Provider::ProtonDrive => RcloneConfigItem::ProtonDrive {
                                username: widget_values.remove(&LoginField::Username).unwrap(),
                                password: widget_values.remove(&LoginField::Password).unwrap(),
                                totp: widget_values.remove(&LoginField::Totp).unwrap(),
                            },
                            Provider::WebDav => RcloneConfigItem::WebDav {
                                url: widget_values.remove(&LoginField::Url).unwrap(),
                                vendor: WebDavVendors::WebDav,
                                user: widget_values.remove(&LoginField::Username).unwrap(),
                                pass: widget_values.remove(&LoginField::Password).unwrap(),
                            },
                        },
                    )
                    .await
                    .unwrap();

                    // Verify that we can login by listing a file on the remote, removing the config
                    // entry if we can't.
                    if let Err(err) = rclone::sync::stat(&config_name, "/").await {
                        rclone::sync::delete_config(&config_name).await.unwrap();
                        let error = err.error;

                        return Err(
                            if error.contains("Temporary failure in name resolution")
                                || error.contains("no such host")
                            {
                                LoginCommandErr::NameResolution
                            } else if error.contains("The password is not correct")
                                || error.contains("Incorrect login credentials")
                                || error.contains("password was incorrect")
                            {
                                LoginCommandErr::InvalidPassword
                            } else if error.contains("this account requires a 2FA code") {
                                LoginCommandErr::MissingTotp
                            } else {
                                LoginCommandErr::ValidityUnknown(error)
                            },
                        );
                    }

                    // We're good to go, so return the config name to send to the parent widget.
                    Ok(config_name)
                });

                if needs_auth_window {
                    self.auth_sender = Some(tx);
                }
            }
            LoginMsg::CancelAuthenticate => {
                self.auth_sender.take().unwrap().send(()).await.unwrap();
            }
        }

        self.update_view(widgets, sender);
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        self.prep_auth.emit(AlertMsg::Hide);
        self.auth.emit(AlertMsg::Hide);
        self.show_login_spinner = false;

        let server_name = if self.provider.is_webdav() {
            "the server"
        } else {
            self.provider.clone().into()
        };

        match message {
            Ok(remote_name) => sender.output(LoginOutput::NewLogin(remote_name)).unwrap(),
            Err(err) => match err {
                // Everything that needs to be handled is done in the code above and below this
                // `match` statement.
                LoginCommandErr::Cancelled => (),
                // TODO: Both of these should use Relm4 components, but we're gonna see if we can
                // get [`Alert`] from `relm4_components` to use `adw::MessageDialog` first.
                LoginCommandErr::AuthServer(err) => gtk_util::show_codeblock_error(
                    &tr::tr!("Unable to start authentication server"),
                    Some(&tr::tr!(
                        "More information about the error is included below"
                    )),
                    &err,
                ),
                LoginCommandErr::Token(err) => gtk_util::show_codeblock_error(
                    &tr::tr!("Unable to obtain token"),
                    Some(&tr::tr!(
                        "More information about the error is included below"
                    )),
                    &err,
                ),
                LoginCommandErr::NameResolution => {
                    let mut alert_state = self.alert.state().get_mut();
                    let mut settings = &mut alert_state.model.settings;
                    settings.text = Some(tr::tr!("Authentication error"));
                    settings.secondary_text = Some(tr::tr!("Celeste was unable to connect to {}. Check your internet connection and try again.", server_name));
                    self.alert.emit(AlertMsg::Show);
                },
                LoginCommandErr::InvalidPassword => {
                    let mut alert_state = self.alert.state().get_mut();
                    let mut settings = &mut alert_state.model.settings;
                    settings.text = Some(tr::tr!("Authentication error"));
                    settings.secondary_text = Some(tr::tr!("An invalid password was entered for the given username. Check your login credentials and try again."));
                    self.alert.emit(AlertMsg::Show);
                }
                LoginCommandErr::MissingTotp => {
                    let mut alert_state = self.alert.state().get_mut();
                    let mut settings = &mut alert_state.model.settings;
                    settings.text = Some(tr::tr!("Authentication error"));
                    settings.secondary_text = Some(tr::tr!("A 2FA code is required to log in to this account. Provide one and try again."));
                    self.alert.emit(AlertMsg::Show)
                },
                LoginCommandErr::ValidityUnknown(err) => gtk_util::show_codeblock_error(
                    &tr::tr!("Authentication error"),
                    Some(&tr::tr!("Celeste was unable to authenticate to {}. The information below may help with find out why.", server_name)),
                    &err,
                )
            },
        }

        root.set_sensitive(true);
    }
}
