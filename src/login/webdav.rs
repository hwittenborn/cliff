//! The data for a WebDAV Rclone config.
use super::{login_util, nextcloud::NextcloudConfig, owncloud::OwncloudConfig, ServerType};
use crate::mpsc::Sender;
use adw::{
    gtk::{glib, Button, Widget},
    prelude::*,
    ApplicationWindow,
};

pub enum WebDavType {
    Nextcloud,
    Owncloud,
    WebDav,
}

#[derive(Clone, Debug, Default)]
pub struct WebDavConfig {
    pub server_name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
}

impl super::LoginTrait for WebDavConfig {
    fn get_sections(
        _window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
    ) -> (Vec<Widget>, Button) {
        Self::webdav_sections(sender, WebDavType::WebDav)
    }
}

impl WebDavConfig {
    pub fn webdav_sections(
        sender: Sender<Option<ServerType>>,
        webdav_type: WebDavType,
    ) -> (Vec<Widget>, Button) {
        let mut sections: Vec<Widget> = vec![];

        let server_name = login_util::server_name_input();
        let server_url = login_util::server_url_input(match webdav_type {
            WebDavType::Nextcloud | WebDavType::Owncloud => true,
            WebDavType::WebDav => false,
        });
        let username = login_util::username_input();
        let password = login_util::password_input();
        let submit_button = login_util::submit_button();

        sections.push(server_name.clone().into());
        sections.push(server_url.clone().into());
        sections.push(username.clone().into());
        sections.push(password.clone().into());

        submit_button.connect_clicked(
            glib::clone!(@weak server_name, @weak server_url, @weak username, @weak password => move |_| {
                // Nextcloud/Owncloud server types have everything after 'remote.php' stripped, so
                // add it back here.
                let formatted_nextcloud_url = format!("{}/remote.php/dav/files/{}", server_url.text(), username.text());

                let server_type = match webdav_type {
                    WebDavType::Nextcloud => ServerType::Nextcloud(NextcloudConfig {
                        server_name: server_name.text().to_string(),
                        server_url: formatted_nextcloud_url,
                        username: username.text().to_string(),
                        password: password.text().to_string(),
                    }),
                    WebDavType::Owncloud => ServerType::Owncloud(OwncloudConfig {
                        server_name: server_name.text().to_string(),
                        server_url: formatted_nextcloud_url,
                        username: username.text().to_string(),
                        password: password.text().to_string(),
                    }),
                    WebDavType::WebDav => ServerType::WebDav(WebDavConfig{
                        server_name: server_name.text().to_string(),
                        server_url: server_url.text().to_string(),
                        username: username.text().to_string(),
                        password: password.text().to_string(),
                    })
                };
                sender.send(Some(server_type));
            }),
        );

        server_name.connect_changed(glib::clone!(@weak server_name, @weak server_url, @weak username, @weak password, @weak submit_button => move |_| login_util::check_responses(&[&server_name, &server_url, &username, &password.into()], &submit_button)));
        server_url.connect_changed(glib::clone!(@weak server_name, @weak server_url, @weak username, @weak password, @weak submit_button => move |_| login_util::check_responses(&[&server_name, &server_url, &username, &password.into()], &submit_button)));
        username.connect_changed(glib::clone!(@weak server_name, @weak server_url, @weak username, @weak password, @weak submit_button => move |_| login_util::check_responses(&[&server_name, &server_url, &username, &password.into()], &submit_button)));
        password.connect_changed(glib::clone!(@weak server_name, @weak server_url, @weak username, @weak password, @weak submit_button => move |_| login_util::check_responses(&[&server_name, &server_url, &username, &password.into()], &submit_button)));

        (sections, submit_button)
    }
}
