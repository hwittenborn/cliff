//! The data for a Dropbox Rclone config.
use super::ServerType;
use crate::{
    login::gdrive::{AuthType, GDriveConfig},
    mpsc::Sender,
};
use adw::{gtk::{Button, Widget}, ApplicationWindow};

static DEFAULT_CLIENT_ID: &str = "hke0fgr43viaq03";
static DEFAULT_CLIENT_SECRET: &str = "o4cpx8trcnneq7a";

#[derive(Clone, Debug, Default)]
pub struct DropboxConfig {
    pub server_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub auth_json: String,
}

impl super::LoginTrait for DropboxConfig {
    fn get_sections(
        window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
    ) -> (Vec<Widget>, Button) {
        GDriveConfig::auth_sections(
            window,
            sender,
            AuthType::Dropbox,
            DEFAULT_CLIENT_ID.to_owned(),
            DEFAULT_CLIENT_SECRET.to_owned(),
        )
    }
}
