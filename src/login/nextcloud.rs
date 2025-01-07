//! The data for a Nextcloud Rclone config.
use super::{
    webdav::{WebDavConfig, WebDavType},
    ServerType,
};
use crate::mpsc::Sender;
use adw::{gtk::{Button, Widget}, ApplicationWindow};

#[derive(Clone, Debug, Default)]
pub struct NextcloudConfig {
    pub server_name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
}

impl super::LoginTrait for NextcloudConfig {
    fn get_sections(
        _window: &ApplicationWindow,
        sender: Sender<Option<ServerType>>,
    ) -> (Vec<Widget>, Button) {
        WebDavConfig::webdav_sections(sender, WebDavType::Nextcloud)
    }
}
