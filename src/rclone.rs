//! Structs and functions for use with Rclone RPC calls.
use crate::util;
use relm4::{
    adw::{ffi::ADW_ANIMATION_PLAYING, glib},
    gtk::glib::application_name,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use time::OffsetDateTime;

/// Get a remote from the config file.
pub async fn get_remote<T: ToString>(remote: T) -> Option<Remote> {
    let remote = remote.to_string();

    let config_str = relm4::spawn_blocking(glib::clone!(@strong remote => move || librclone::rpc(
        "config/get",
        json!({
            "name": remote
        })
        .to_string(),
    )))
    .await
    .unwrap()
    .unwrap();
    let config: HashMap<String, String> = serde_json::from_str(&config_str).unwrap();

    match config["type"].as_str() {
        "dropbox" => Some(Remote::Dropbox(DropboxRemote {
            remote_name: remote,
            client_id: config["client_id"].clone(),
            client_secret: config["client_secret"].clone(),
        })),
        "drive" => Some(Remote::GDrive(GDriveRemote {
            remote_name: remote,
            client_id: config["client_id"].clone(),
            client_secret: config["client_secret"].clone(),
        })),
        "pcloud" => Some(Remote::PCloud(PCloudRemote {
            remote_name: remote,
            client_id: config["client_id"].clone(),
            client_secret: config["client_secret"].clone(),
        })),
        "protondrive" => Some(Remote::ProtonDrive(ProtonDriveRemote {
            remote_name: remote,
            username: config["username"].clone(),
        })),
        "webdav" => {
            let vendor = match config["vendor"].as_str() {
                "nextcloud" => WebDavVendors::Nextcloud,
                "owncloud" => WebDavVendors::Owncloud,
                "webdav" => WebDavVendors::WebDav,
                _ => unreachable!(),
            };

            Some(Remote::WebDav(WebDavRemote {
                remote_name: remote,
                user: config["user"].clone(),
                pass: config["pass"].clone(),
                url: config["user"].clone(),
                vendor,
            }))
        }
        _ => None,
    }
}

/// Get all the remotes from the config file.
pub async fn get_remotes() -> Vec<Remote> {
    let configs_str =
        relm4::spawn_blocking(|| librclone::rpc("config/listremotes", json!({}).to_string()))
            .await
            .unwrap()
            .unwrap();
    let configs = {
        let config: HashMap<String, Vec<String>> = serde_json::from_str(&configs_str).unwrap();
        config.get(&"remotes".to_string()).unwrap().to_owned()
    };
    let mut celeste_configs = vec![];

    for config in configs {
        celeste_configs.push(get_remote(&config).await.unwrap());
    }

    celeste_configs
}

/// The types of remotes in the config.
#[derive(Clone)]
pub enum Remote {
    Dropbox(DropboxRemote),
    GDrive(GDriveRemote),
    PCloud(PCloudRemote),
    ProtonDrive(ProtonDriveRemote),
    WebDav(WebDavRemote),
}

impl Remote {
    pub fn remote_name(&self) -> String {
        match self {
            Remote::Dropbox(remote) => remote.remote_name.clone(),
            Remote::GDrive(remote) => remote.remote_name.clone(),
            Remote::PCloud(remote) => remote.remote_name.clone(),
            Remote::ProtonDrive(remote) => remote.remote_name.clone(),
            Remote::WebDav(remote) => remote.remote_name.clone(),
        }
    }
}

// The Dropbox remote type.
#[derive(Clone, Debug)]
pub struct DropboxRemote {
    /// The name of the remote.
    pub remote_name: String,
    /// The client id.
    pub client_id: String,
    /// The client secret.
    pub client_secret: String,
}

// The Google Drive remote type.
#[derive(Clone, Debug)]
pub struct GDriveRemote {
    /// The name of the remote.
    pub remote_name: String,
    /// The client id.
    pub client_id: String,
    /// The client secret.
    pub client_secret: String,
}

// The pCloud remote type.
#[derive(Clone, Debug)]
pub struct PCloudRemote {
    /// The name of the remote.
    pub remote_name: String,
    /// The client id.
    pub client_id: String,
    /// The client secret.
    pub client_secret: String,
}

// The Proton Drive remote type.
#[derive(Clone, Debug)]
pub struct ProtonDriveRemote {
    /// The name of the remote.
    pub remote_name: String,
    /// the username.
    pub username: String,
}

// The WebDav remote type.
#[derive(Clone, Debug)]
pub struct WebDavRemote {
    /// The name of the remote.
    pub remote_name: String,
    /// The username for the remote.
    pub user: String,
    /// The password for the remote.
    pub pass: String,
    /// The URL for the remote.
    pub url: String,
    /// The vendor of the remote.
    pub vendor: WebDavVendors,
}

/// Possible WebDav vendors.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WebDavVendors {
    Nextcloud,
    Owncloud,
    WebDav,
}

impl ToString for WebDavVendors {
    fn to_string(&self) -> String {
        match self {
            Self::Nextcloud => "Nextcloud",
            Self::Owncloud => "Owncloud",
            Self::WebDav => "WebDav",
        }
        .to_string()
    }
}

/// A remote in the Rclone config.
#[derive(Serialize)]
pub enum RcloneConfigItem {
    Dropbox {
        client_id: String,
        client_secret: String,
        token: String,
    },
    GoogleDrive {
        client_id: String,
        client_secret: String,
        token: String,
    },
    PCloud {
        client_id: String,
        client_secret: String,
        token: String,
    },
    ProtonDrive {
        username: String,
        password: String,
        totp: String,
    },
    WebDav {
        url: String,
        vendor: WebDavVendors,
        user: String,
        pass: String,
    },
}

impl RcloneConfigItem {
    /// Convert this item into json suitable for "config/create" from librclone.
    ///
    /// `name` is the label used for the remote in the rclone config.
    fn config_json(&self, name: &str) -> Value {
        match self {
            Self::Dropbox {
                client_id,
                client_secret,
                token,
            } => json!({
                "name": name,
                "type": "dropbox",
                "parameters": {
                    "client_id": client_id,
                    "client_secret": client_secret,
                    "token": token,
                    "config_refresh_token": true
                },
                "opt": {
                    "obscure": true
                }
            }),
            Self::GoogleDrive {
                client_id,
                client_secret,
                token,
            } => json!({
                "name": name,
                "type": "drive",
                "parameters": {
                    "client_id": client_id,
                    "client_secret": client_secret,
                    "token": token,
                    "config_refresh_token": true
                },
                "opt": {
                    "obscure": true
                }
            }),
            Self::PCloud {
                client_id,
                client_secret,
                token,
            } => json!({
                "name": name,
                "type": "pcloud",
                "parameters": {
                    "client_id": client_id,
                    "client_secret": client_secret,
                    "token": token,
                    "config_refresh_token": true
                },
                "opt": {
                    "obscure": true
                }
            }),
            Self::ProtonDrive {
                username,
                password,
                totp,
            } => json!({
                "name": name,
                "type": "protondrive",
                "parameters": {
                    "username": username,
                    "password": password,
                    "2fa": totp
                },
                "opt": {
                    "obscure": true
                }
            }),
            Self::WebDav {
                url,
                vendor,
                user,
                pass,
            } => json!({
                "name": name,
                "type": "webdav",
                "parameters": {
                    "url": url,
                    "vendor": vendor,
                    "user": user,
                    "pass": pass
                },
                "opt": {
                    "obscure": true
                }
            }),
        }
    }
}

/// Error returned from Rclone.
#[derive(Clone, Deserialize, Debug)]
pub struct RcloneError {
    pub error: String,
}

/// The output of an `operations/stat` command.
#[derive(Clone, Deserialize, Debug)]
pub struct RcloneStat {
    item: Option<RcloneRemoteItem>,
}

/// The output of an `operations/list` command.
#[derive(Clone, Deserialize, Debug)]
pub struct RcloneList {
    #[serde(rename = "list")]
    list: Vec<RcloneRemoteItem>,
}

/// The list of items in a folder, from the `list` object in the output of the
/// `operations/list` command.
#[derive(Clone, Deserialize, Debug)]
pub struct RcloneRemoteItem {
    #[serde(rename = "IsDir")]
    pub is_dir: bool,
    #[serde(rename = "Path")]
    pub path: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "ModTime", with = "time::serde::rfc3339")]
    pub mod_time: OffsetDateTime,
}

/// The types of items to show in an `operations/list` command.
#[derive(Clone, Debug)]
pub enum RcloneListFilter {
    /// Return all items.
    All,
    /// Only return directories.
    Dirs,
    /// Only return files.
    #[allow(dead_code)]
    Files,
}

/// Functions for syncing to a remote.
pub mod sync {
    use super::{
        RcloneConfigItem, RcloneError, RcloneList, RcloneListFilter, RcloneRemoteItem, RcloneStat,
    };
    use crate::util;
    use serde_json::json;

    /// Get a remote name.
    fn get_remote_name(remote: &str) -> String {
        if remote.ends_with(':') {
            panic!("Remote '{remote}' is not allowed to end with a ':'. Please omit it.",);
        }
        format!("{remote}:")
    }

    /// Common function for some of the below command.
    async fn common(command: &str, remote_name: &str, path: &str) -> Result<(), RcloneError> {
        let command = command.to_string();
        let remote_name = remote_name.to_string();
        let path = path.to_string();

        let resp = relm4::spawn_blocking(move || {
            librclone::rpc(
                command,
                &json!({
                    "fs": get_remote_name(&remote_name),
                    "remote": util::strip_slashes(&path),
                })
                .to_string(),
            )
        })
        .await
        .unwrap();

        match resp {
            Ok(_) => Ok(()),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// Create a config.
    pub async fn create_config(name: &str, config: RcloneConfigItem) -> Result<(), RcloneError> {
        let name = name.to_string();

        let resp = relm4::spawn_blocking(move || {
            librclone::rpc("config/create", config.config_json(&name).to_string())
        })
        .await
        .unwrap();

        match resp {
            Ok(_) => Ok(()),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// Delete a config.
    pub async fn delete_config(remote_name: &str) -> Result<(), RcloneError> {
        let remote_name = remote_name.to_string();

        let resp = relm4::spawn_blocking(move || {
            librclone::rpc("config/delete", &json!({ "name": remote_name }).to_string())
        })
        .await
        .unwrap();

        match resp {
            Ok(_) => Ok(()),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// Get statistics about a file or folder.
    pub async fn stat(
        remote_name: &str,
        path: &str,
    ) -> Result<Option<RcloneRemoteItem>, RcloneError> {
        let remote_name = remote_name.to_string();
        let path = path.to_string();

        let resp = relm4::spawn_blocking(move || {
            librclone::rpc(
                "operations/stat",
                &json!({
                    "fs": get_remote_name(&remote_name),
                    "remote": util::strip_slashes(&path)
                })
                .to_string(),
            )
        })
        .await
        .unwrap();

        match resp {
            Ok(json_str) => Ok(serde_json::from_str::<RcloneStat>(&json_str).unwrap().item),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// List the files/folders in a path.
    pub async fn list(
        remote_name: &str,
        path: &str,
        recursive: bool,
        filter: RcloneListFilter,
    ) -> Result<Vec<RcloneRemoteItem>, RcloneError> {
        let remote_name = remote_name.to_string();
        let path = path.to_string();

        let opts = match filter {
            RcloneListFilter::All => json!({ "recurse": recursive }),
            RcloneListFilter::Dirs => json!({"dirsOnly": true, "recurse": recursive}),
            RcloneListFilter::Files => json!({"filesOnly": true, "recurse": recursive}),
        };

        let resp = relm4::spawn_blocking(move || {
            librclone::rpc(
                "operations/list",
                &json!({
                    "fs": get_remote_name(&remote_name),
                    "remote": util::strip_slashes(&path),
                    "opt": opts
                })
                .to_string(),
            )
        })
        .await
        .unwrap();

        match resp {
            Ok(json_str) => Ok(serde_json::from_str::<RcloneList>(&json_str).unwrap().list),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// make a directory on the remote.
    pub async fn mkdir(remote_name: &str, path: &str) -> Result<(), RcloneError> {
        common("operations/mkdir", remote_name, path).await
    }

    /// Delete a file.
    pub async fn delete(remote_name: &str, path: &str) -> Result<(), RcloneError> {
        common("operations/delete", remote_name, path).await
    }
    /// Remove a directory and all of its contents.
    pub async fn purge(remote_name: &str, path: &str) -> Result<(), RcloneError> {
        common("operations/purge", remote_name, path).await
    }

    /// Utility for copy functions.
    async fn copy(
        src_fs: &str,
        src_remote: &str,
        dst_fs: &str,
        dst_remote: &str,
    ) -> Result<(), RcloneError> {
        let src_fs = src_fs.to_string();
        let src_remote = src_remote.to_string();
        let dst_fs = dst_fs.to_string();
        let dst_remote = dst_remote.to_string();

        let resp = relm4::spawn_blocking(move || {
            librclone::rpc(
                "operations/copyfile",
                &json!({
                    "srcFs": src_fs,
                    "srcRemote": util::strip_slashes(&src_remote),
                    "dstFs": dst_fs,
                    "dstRemote": util::strip_slashes(&dst_remote)
                })
                .to_string(),
            )
        })
        .await
        .unwrap();

        match resp {
            Ok(_) => Ok(()),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// Copy a file from the local machine to the remote.
    pub async fn copy_to_remote(
        local_file: &str,
        remote_name: &str,
        remote_destination: &str,
    ) -> Result<(), RcloneError> {
        copy(
            "/",
            local_file,
            &get_remote_name(remote_name),
            remote_destination,
        )
        .await
    }

    /// Copy a file from the remote to the local machine.
    pub async fn copy_to_local(
        local_destination: &str,
        remote_name: &str,
        remote_file: &str,
    ) -> Result<(), RcloneError> {
        copy(
            &get_remote_name(remote_name),
            remote_file,
            "/",
            local_destination,
        )
        .await
    }
}
