//! The data for a OneDrive Rclone config.
use super::ServerType;
use crate::{
    login::gdrive::{AuthType, GDriveConfig},
    mpsc::Sender,
};
use adw::{gtk::Button, ApplicationWindow, EntryRow};

static DEFAULT_CLIENT_ID: &str = "0914e4da-c4f6-4bf3-ab2a-458d93b39a36";
static DEFAULT_CLIENT_SECRET: &str = "oj18Q~sUrYTSQxQhdbBlXwwTggHKYjKbRES6HcHj";

#[derive(Clone, Debug, Default)]
pub struct OneDriveConfig {
    pub server_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub auth_json: String,
}

impl super::LoginTrait for OneDriveConfig {
    fn get_sections(
        window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
    ) -> (Vec<EntryRow>, Button) {
        GDriveConfig::auth_sections(
            window,
            sender,
            AuthType::OneDrive,
            DEFAULT_CLIENT_ID.to_owned(),
            DEFAULT_CLIENT_SECRET.to_owned(),
        )
    }
}
