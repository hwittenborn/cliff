//! The data for a Proton Drive Rclone config.
use super::ServerType;
use crate::{login::login_util, mpsc::Sender};
use adw::{glib, gtk::{Button, Widget}, prelude::*, ApplicationWindow};

#[derive(Clone, Debug, Default)]
pub struct ProtonDriveConfig {
    pub server_name: String,
    pub username: String,
    pub password: String,
    pub totp: String,
}

impl super::LoginTrait for ProtonDriveConfig {
    fn get_sections(
        _window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
    ) -> (Vec<Widget>, Button) {
        let mut sections = vec![];
        let server_name = login_util::server_name_input();
        let username = login_util::username_input();
        let password = login_util::password_input();
        let totp = login_util::totp_input();
        let submit_button = login_util::submit_button();

        sections.push(server_name.clone().into());
        sections.push(username.clone().into());
        sections.push(password.clone().into());
        sections.push(totp.clone().into());

        submit_button.connect_clicked(
            glib::clone!(@weak server_name, @weak username, @weak password, @weak totp => move |_| {
                sender.send(Some(ServerType::ProtonDrive(ProtonDriveConfig {
                    server_name: server_name.text().to_string(),
                    username: username.text().to_string(),
                    password: password.text().to_string(),
                    totp: totp.text().to_string()
                })));
            }),
        );

        server_name.connect_changed(glib::clone!(@weak server_name, @weak username, @weak password, @weak totp, @weak submit_button => move |_| login_util::check_responses(&[&server_name, &username, &password.into(), &totp], &submit_button)));
        username.connect_changed(glib::clone!(@weak server_name, @weak username, @weak password, @weak totp, @weak submit_button => move |_| login_util::check_responses(&[&server_name, &username, &password.into(), &totp], &submit_button)));
        password.connect_changed(glib::clone!(@weak server_name, @weak username, @weak password, @weak totp, @weak submit_button => move |_| login_util::check_responses(&[&server_name, &username, &password.into(), &totp], &submit_button)));
        totp.connect_changed(glib::clone!(@weak server_name, @weak username, @weak password, @weak totp, @weak submit_button => move |_| login_util::check_responses(&[&server_name, &username, &password.into(), &totp], &submit_button)));

        (sections, submit_button)
    }
}
