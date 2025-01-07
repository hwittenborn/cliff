//! Structs and functions for use with Rclone RPC calls.
use crate::util;
use adw::glib;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use time::OffsetDateTime;

/// Get a remote from the config file.
pub fn get_remote<T: ToString>(remote: T) -> Option<Remote> {
    let remote = remote.to_string();

    let config_str = util::run_in_background(
        glib::clone!(@strong remote => move || librclone::rpc("config/get", json!({
            "name": remote
        }).to_string()).unwrap()),
    );
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
            hostname: config["hostname"].clone(),
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
pub fn get_remotes() -> Vec<Remote> {
    let configs_str = util::run_in_background(move || {
        librclone::rpc("config/listremotes", json!({}).to_string())
            .unwrap_or_else(|_| unreachable!())
    });
    let configs = {
        let config: HashMap<String, Vec<String>> = serde_json::from_str(&configs_str).unwrap();
        config.get(&"remotes".to_string()).unwrap().to_owned()
    };
    let mut celeste_configs = vec![];

    for config in configs {
        celeste_configs.push(get_remote(&config).unwrap());
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
    /// The API server to connect to (NA or EU)
    pub hostname: String,
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
#[derive(Clone, Debug)]
pub enum WebDavVendors {
    Nextcloud,
    Owncloud,
    GDrive,
    PCloud,
    WebDav,
}

impl ToString for WebDavVendors {
    fn to_string(&self) -> String {
        match self {
            Self::Nextcloud => "Nextcloud",
            Self::Owncloud => "Owncloud",
            Self::GDrive => "Google Drive",
            Self::PCloud => "pCloud",
            Self::WebDav => "WebDav",
        }
        .to_string()
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
/// All functions in this module automatically run under
/// [`util::run_in_background`], so they don't need to be wrapped around
/// such to be ran during UI execution.
pub mod sync {
    use super::{RcloneError, RcloneList, RcloneListFilter, RcloneRemoteItem, RcloneStat};
    use crate::util;
    use serde_json::json;

    /// Get a remote name.
    fn get_remote_name(remote: &str) -> String {
        if remote.ends_with(':') {
            panic!("Remote '{remote}' is not allowed to end with a ':'. Please omit it.",);
        }
        format!("{remote}:")
    }

    /// Run an Rclone command without blocking the GUI.
    fn run<T: ToString>(method: T, input: T) -> Result<String, String> {
        let method = method.to_string();
        let input = input.to_string();
        util::run_in_background(|| librclone::rpc(method, input))
    }

    /// Common function for some of the below command.
    fn common(command: &str, remote_name: &str, path: &str) -> Result<(), RcloneError> {
        let resp = run(
            command,
            &json!({
                "fs": get_remote_name(remote_name),
                "remote": util::strip_slashes(path),
            })
            .to_string(),
        );

        match resp {
            Ok(_) => Ok(()),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// Delete a config.
    pub fn delete_config(remote_name: &str) -> Result<(), RcloneError> {
        let resp = run("config/delete", &json!({ "name": remote_name }).to_string());

        match resp {
            Ok(_) => Ok(()),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// Get statistics about a file or folder.
    pub fn stat(remote_name: &str, path: &str) -> Result<Option<RcloneRemoteItem>, RcloneError> {
        let resp = run(
            "operations/stat",
            &json!({
                "fs": get_remote_name(remote_name),
                "remote": util::strip_slashes(path)
            })
            .to_string(),
        );

        match resp {
            Ok(json_str) => Ok(serde_json::from_str::<RcloneStat>(&json_str).unwrap().item),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// List the files/folders in a path.
    pub fn list(
        remote_name: &str,
        path: &str,
        recursive: bool,
        filter: RcloneListFilter,
    ) -> Result<Vec<RcloneRemoteItem>, RcloneError> {
        let opts = match filter {
            RcloneListFilter::All => json!({ "recurse": recursive }),
            RcloneListFilter::Dirs => json!({"dirsOnly": true, "recurse": recursive}),
            RcloneListFilter::Files => json!({"filesOnly": true, "recurse": recursive}),
        };

        let resp = run(
            "operations/list",
            &json!({
                "fs": get_remote_name(remote_name),
                "remote": util::strip_slashes(path),
                "opt": opts
            })
            .to_string(),
        );

        match resp {
            Ok(json_str) => Ok(serde_json::from_str::<RcloneList>(&json_str).unwrap().list),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// make a directory on the remote.
    pub fn mkdir(remote_name: &str, path: &str) -> Result<(), RcloneError> {
        common("operations/mkdir", remote_name, path)
    }

    /// Delete a file.
    pub fn delete(remote_name: &str, path: &str) -> Result<(), RcloneError> {
        common("operations/delete", remote_name, path)
    }
    /// Remove a directory and all of its contents.
    pub fn purge(remote_name: &str, path: &str) -> Result<(), RcloneError> {
        common("operations/purge", remote_name, path)
    }

    /// Utility for copy functions.
    fn copy(
        src_fs: &str,
        src_remote: &str,
        dst_fs: &str,
        dst_remote: &str,
    ) -> Result<(), RcloneError> {
        let resp = run(
            "operations/copyfile",
            &json!({
                "srcFs": src_fs,
                "srcRemote": util::strip_slashes(src_remote),
                "dstFs": dst_fs,
                "dstRemote": util::strip_slashes(dst_remote)
            })
            .to_string(),
        );

        match resp {
            Ok(_) => Ok(()),
            Err(json_str) => Err(serde_json::from_str(&json_str).unwrap()),
        }
    }

    /// Copy a file from the local machine to the remote.
    pub fn copy_to_remote(
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
    }

    /// Copy a file from the remote to the local machine.
    pub fn copy_to_local(
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
    }
}
