//! Functions and libcelesteities for logging in to a server.
use crate::{
    entities::{RemotesActiveModel, RemotesModel},
    gtk_util,
    mpsc::{self, Sender},
    rclone,
    traits::prelude::*,
    util,
};
mod dropbox;
mod gdrive;
pub mod login_util;
mod nextcloud;
mod owncloud;
mod pcloud;
mod proton_drive;
mod webdav;

use adw::{
    glib,
    gtk::{Box, Button, Inhibit, ListBox, Orientation, SelectionMode, StringList},
    prelude::*,
    Application, ApplicationWindow, ComboRow, EntryRow, HeaderBar,
};
use dropbox::DropboxConfig;
use gdrive::GDriveConfig;
use nextcloud::NextcloudConfig;
use owncloud::OwncloudConfig;
use pcloud::PCloudConfig;
use proton_drive::ProtonDriveConfig;
use std::{cell::RefCell, rc::Rc};
use webdav::WebDavConfig;

use sea_orm::{entity::prelude::*, ActiveValue, DatabaseConnection};
use serde_json::json;

/// A trait to get some data from configs.
trait LoginTrait {
    fn get_sections(
        window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
    ) -> (Vec<EntryRow>, Button);
}

/// An enum representing valid storage types.
#[derive(Clone, Debug)]
pub enum ServerType {
    Dropbox(dropbox::DropboxConfig),
    GDrive(gdrive::GDriveConfig),
    Nextcloud(nextcloud::NextcloudConfig),
    Owncloud(owncloud::OwncloudConfig),
    PCloud(pcloud::PCloudConfig),
    ProtonDrive(proton_drive::ProtonDriveConfig),
    WebDav(webdav::WebDavConfig),
}

impl ToString for ServerType {
    fn to_string(&self) -> String {
        match self {
            Self::Dropbox(_) => "Dropbox",
            Self::GDrive(_) => "Google Drive",
            Self::Nextcloud(_) => "Nextcloud",
            Self::Owncloud(_) => "Owncloud",
            Self::PCloud(_) => "pCloud",
            Self::ProtonDrive(_) => "Proton Drive",
            Self::WebDav(_) => "WebDAV",
        }
        .to_string()
    }
}

// Verify if a specific config can log in to a server.
pub fn can_login(_app: &Application, config_name: &str) -> bool {
    if let Err(err) = rclone::sync::stat(config_name, "/") {
        let err_msg = if err.error.contains("Temporary failure in name resolution") {
            tr::tr!(
                "Unable to connect to the server. Check your internet connection and try again."
            )
        } else if err.error.contains("this account requires a 2FA code") {
            tr::tr!("A 2FA code is required to log in to this account. Provide one and try again.")
        } else {
            tr::tr!(
                "Unable to authenticate to the server. Check your login credentials and try again."
            )
        };

        gtk_util::show_error(&tr::tr!("Unable to log in"), Some(&err_msg));
        false
    } else {
        true
    }
}

/// Create a new session. Returns [`Some`] with the new session if the client
/// successfully logged in, and [`None`] on other events, such as closing the
/// window before logging in. Logged in clients can be obtained after this point
/// via [`rclone::get_configs`].
pub fn login(app: &Application, db: &DatabaseConnection) -> Option<RemotesModel> {
    // The mspc sender/receiver to get data from fields.
    let (sender, mut receiver) = mpsc::channel::<Option<ServerType>>();

    // The window.
    let window = ApplicationWindow::builder()
        .application(app)
        .title(&util::get_title!("Log in"))
        .width_request(400)
        .build();
    window.add_css_class("celeste-global-padding");
    window.connect_close_request(glib::clone!(@strong sender => move |_| {
        sender.send(None);
        Inhibit(false)
    }));

    // The stack containing the forms for all login sections.
    let dropbox_name = ServerType::Dropbox(Default::default()).to_string();
    let gdrive_name = ServerType::GDrive(Default::default()).to_string();
    let nextcloud_name = ServerType::Nextcloud(Default::default()).to_string();
    let owncloud_name = ServerType::Owncloud(Default::default()).to_string();
    let pcloud_name = ServerType::PCloud(Default::default()).to_string();
    let proton_drive_name = ServerType::ProtonDrive(Default::default()).to_string();
    let webdav_name = ServerType::WebDav(Default::default()).to_string();

    // The dropdown for selecting the server type.
    let server_type_dropdown = ComboRow::builder().title(&tr::tr!("Server Type")).build();
    let server_types_array = [
        dropbox_name.as_str(),
        gdrive_name.as_str(),
        nextcloud_name.as_str(),
        owncloud_name.as_str(),
        pcloud_name.as_str(),
        proton_drive_name.as_str(),
        webdav_name.as_str(),
    ];
    let server_types = StringList::new(&server_types_array);
    server_type_dropdown.set_model(Some(&server_types));

    // A box containing the header bar and input sections.
    let container = Box::builder().orientation(Orientation::Vertical).build();
    let input_sections = ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .build();
    container.append(&HeaderBar::new());
    container.append(&input_sections);
    input_sections.append(&server_type_dropdown);

    // Set up the submit button.
    let submit_button = login_util::submit_button();
    container.append(&submit_button);

    // Get the window items for each server type.
    let dropbox_items = DropboxConfig::get_sections(&window, sender.clone());
    let gdrive_items = GDriveConfig::get_sections(&window, sender.clone());
    let nextcloud_items = NextcloudConfig::get_sections(&window, sender.clone());
    let owncloud_items = OwncloudConfig::get_sections(&window, sender.clone());
    let pcloud_items = PCloudConfig::get_sections(&window, sender.clone());
    let proton_drive_items = ProtonDriveConfig::get_sections(&window, sender.clone());
    let webdav_items = WebDavConfig::get_sections(&window, sender);

    // Store the active items.
    let active_items: Rc<RefCell<(Vec<EntryRow>, Button)>> =
        Rc::new(RefCell::new((vec![], submit_button)));

    // Configure the window to change the widgets when the selected server type
    // changes.
    server_type_dropdown.connect_selected_notify(glib::clone!(@weak container, @weak input_sections, @strong server_types, @strong nextcloud_items, @strong webdav_items, @strong active_items => move |server_type_dropdown| {
        let server_type = server_types.string(server_type_dropdown.selected()).unwrap().to_string();

        let (rows, submit_button) = match server_type.to_lowercase().as_str() {
            "dropbox" => dropbox_items.clone(),
            "google drive" => gdrive_items.clone(),
            "nextcloud" => nextcloud_items.clone(),
            "owncloud" => owncloud_items.clone(),
            "pcloud" => pcloud_items.clone(),
            "proton drive" => proton_drive_items.clone(),
            "webdav" => webdav_items.clone(),
            _ => unreachable!()
        };

        // Remove the current submit button.
        let mut ptr = active_items.get_mut_ref();
        container.remove(&ptr.1);

        // Now remove the current listbox items.
        for row in ptr.0.clone()  {
            // Reset the row to default styling and text so that when the user goes back it looks like a fresh page.
            row.set_text("");
            row.remove_css_class("error");

            // Actually remove the item.
            input_sections.remove(&row);
            ptr.0.remove(0);
        }

        // Now set the ones for this remove.
        for row in rows {
            input_sections.append(&row);
            ptr.0.push(row);
        }

        // Now set the new submit button.
        container.append(&submit_button);
        ptr.1 = submit_button;
    }));
    // Go back and forth to the first widget so we can initialize our entries.
    server_type_dropdown.set_selected(1);
    server_type_dropdown.set_selected(0);

    // Set up the window and show it.
    window.set_content(Some(&container));
    window.show();

    // Keep receiving values from the windows on the stack until a valid config
    // is found.
    loop {
        // If the user clicks the 'X' button on the window we get a [`None`] value.
        let server = receiver.recv()?;
        window.set_sensitive(false);

        // Create a new config with the requested name.
        let config_name = match &server {
            ServerType::Dropbox(config) => config.server_name.clone(),
            ServerType::GDrive(config) => config.server_name.clone(),
            ServerType::Nextcloud(config) => config.server_name.clone(),
            ServerType::Owncloud(config) => config.server_name.clone(),
            ServerType::PCloud(config) => config.server_name.clone(),
            ServerType::ProtonDrive(config) => config.server_name.clone(),
            ServerType::WebDav(config) => config.server_name.clone(),
        };

        let config_query = match &server {
            ServerType::Dropbox(config) => json!({
                "name": config_name,
                "parameters": {
                    "client_id": config.client_id,
                    "client_secret": config.client_secret,
                    "token": config.auth_json,
                    "config_refresh_token": false
                },
                "type": "dropbox"
            }),
            ServerType::GDrive(config) => json!({
                "name": config_name,
                "parameters": {
                    "client_id": config.client_id,
                    "client_secret": config.client_secret,
                    "token": config.auth_json,
                    "config_refresh_token": false
                },
                "type": "drive"
            }),
            ServerType::Nextcloud(config) => json!({
                "name": config_name,
                "parameters": {
                    "url": config.server_url,
                    "vendor": "nextcloud",
                    "user": config.username,
                    "pass": config.password
                },
                "type": "webdav",
                "opt": {
                    "obscure": true
                }
            }),
            ServerType::Owncloud(config) => json!({
                "name": config_name,
                "parameters": {
                    "url": config.server_url,
                    "vendor": "owncloud",
                    "user": config.username,
                    "pass": config.password
                },
                "type": "webdav",
                "opt": {
                    "obscure": true
                }
            }),
            ServerType::PCloud(config) => json!({
                "name": config_name,
                "parameters": {
                    "hostname": config.hostname,
                    "token": config.auth_json,
                    "config_refresh_token": false
                },
                "type": "pcloud",
                "opt": {
                    "obscure": true
                }
            }),
            ServerType::ProtonDrive(config) => json!({
                "name": config_name,
                "parameters": {
                    "username": config.username,
                    "password": config.password,
                    "2fa": config.totp
                },
                "type": "protondrive",
                "opt": {
                    "obscure": true
                }
            }),
            ServerType::WebDav(config) => json!({
                "name": config_name,
                "parameters": {
                    "url": config.server_url,
                    "vendor": "webdav",
                    "user": config.username,
                    "pass": config.password
                },
                "type": "webdav",
                "opt": {
                    "obscure": true
                }
            }),
        };

        util::run_in_background(move || {
            librclone::rpc("config/create", config_query.to_string()).unwrap()
        });

        // If we can't connect to the server, assume invalid credentials were given,
        // remote the config, and try asking for input again.
        if !can_login(app, &config_name) {
            util::run_in_background(move || {
                librclone::rpc("config/delete", json!({ "name": config_name }).to_string()).unwrap()
            });
            window.set_sensitive(true);
        // We've passed validation otherwise, so add the remote to the db, close
        // the window and return the config.
        } else {
            let model = util::await_future(
                RemotesActiveModel {
                    name: ActiveValue::Set(config_name),
                    ..Default::default()
                }
                .insert(db),
            )
            .unwrap();

            window.close();
            return Some(model);
        }
    }
}
