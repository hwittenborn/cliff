//! The data for a pCloud Rclone config.
use super::ServerType;
use crate::{
    login::gdrive::{AuthType, GDriveConfig},
    mpsc::Sender,
};
use adw::{gtk::Button, ApplicationWindow, EntryRow};

static DEFAULT_CLIENT_ID: &str = "KRzpo46NKb7";
static DEFAULT_CLIENT_SECRET: &str = "g10qvqgWR85lSvEQWlIqCmPYIhwX";

#[derive(Clone, Debug, Default)]
pub struct PCloudConfig {
    pub server_name: String,
    pub hostname: String,
    pub auth_json: String,
}

impl super::LoginTrait for PCloudConfig {
    fn get_sections(
        window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
    ) -> (Vec<EntryRow>, Button) {
        GDriveConfig::auth_sections(
            window,
            sender,
            AuthType::PCloud,
            DEFAULT_CLIENT_ID.to_owned(),
            DEFAULT_CLIENT_SECRET.to_owned(),
        )
    }
}
