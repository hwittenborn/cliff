mod gtk_util;
mod launch;
mod login;
mod migrations;
mod parent;
mod rclone;
mod util;

// A rename that's a bit nicer to use.
use adw::{gio, prelude::*};
use backtrace::Backtrace;
use clap::Parser;
use futures::executor::BlockAwait;
use if_chain::if_chain;
use launch::LaunchModel;
use migrations::{Migrator, MigratorTrait};
use relm4::prelude::*;
use relm4_components::alert::{Alert, AlertMsg, AlertSettings};
pub use relm4_icons::icon_name as icons;
use sea_orm::{Database, DatabaseConnection};
use sentry::{integrations::panic::PanicIntegration, protocol::Event, ClientOptions, Hub, IntoDsn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    env,
    fmt::Debug,
    fs::{self, File},
    panic::{self, PanicInfo},
    path::PathBuf,
    process::{self, ExitCode},
    sync::Arc,
    thread,
    time::Duration,
};
use tokio::{
    runtime::Runtime,
    sync::mpsc,
    task::{self, LocalSet},
};

use crate::util::css::NO_TITLE;

static SENTRY_DSN: &str =
    "https://e2b3dfc44a5b51cf2a8802130b9d1073@o4505099657740288.ingest.sentry.io/4506271301500928";

/// Setup the system to be in a good state for execution.
///
/// Returns a handle to Celeste's [`DatabaseConnection`] on success. Otherwise
/// returns [`None`] after showing the error to the user.
async fn setup() -> Result<DatabaseConnection, (String, String)> {
    // Create the config directory if it doesn't exist.
    let config_dir = util::get_config_dir();

    if_chain! {
        if !config_dir.exists();
        if let Err(err) = fs::create_dir_all(&config_dir);
        then {
            return Err((
                tr::tr!("Unable to create Celeste's config directory"),
                err.to_string()
            ))
        }
    }

    // Create the database file if it doesn't exist.
    let db_path = config_dir.join("celeste.db");

    if_chain! {
        if !db_path.exists();
        if let Err(err) = File::create(&db_path);

        then {
            return Err((
                tr::tr!("Unable to create Celeste's database file"),
                err.to_string()
            ))
        }
    }

    // Connect to the database.
    let db_path = format!("sqlite:/{}", db_path.display());
    let db = match Database::connect(db_path).await {
        Ok(conn) => conn,
        Err(err) => {
            return Err((
                tr::tr!("Unable to connect to Celeste's database"),
                err.to_string(),
            ))
        }
    };

    // Run migrations.
    if let Err(err) = Migrator::up(&db, None).await {
        return Err((
            tr::tr!("Unable to run database migrations"),
            err.to_string(),
        ));
    }

    Ok(db)
}

/// Create a new [`RelmApp`] instance.
fn create_app<T: Debug>() -> RelmApp<T> {
    let app = RelmApp::new(util::APP_ID);
    relm4_icons::initialize_icons();
    relm4::set_global_css(include_str!(concat!(env!("OUT_DIR"), "/style.css")));
    app
}

/// The Sentry panic information passed between Celeste's parent process and GUI
/// process.
#[derive(Serialize, Deserialize)]
struct SentryPanicInfo {
    /// The backtrace.
    backtrace: Backtrace,
    /// The Sentry event.
    event: Event<'static>,
}

#[derive(Parser)]
struct Cli {
    /// Start Celeste in the background.
    #[arg(long)]
    background: bool,

    /// The path to store Sentry's panic information in for the GUI subprocess.
    ///
    /// The text stored in this file should be the JSON representation of
    /// [`SentryPanicInfo`].
    #[arg(env, hide = true)]
    panic_file: Option<PathBuf>,
}

fn start() {
    // Configure Rclone.
    let mut config = util::get_config_dir();
    config.push("rclone.conf");
    librclone::initialize();
    librclone::rpc("config/setpath", json!({ "path": config }).to_string()).unwrap();

    // Run the app.
    create_app()
        .visible_on_activate(false)
        .run_async::<launch::LaunchModel>(());
}

/// Start up the Celeste GUI.
///
/// We don't live in a perfect world, and panics are thus likewise bound to
/// happen at some point. We'd like to show these to the user in the GUI though,
/// and that complicates things a bit.
///
/// The way we currently handle that is by having a main Celeste process that
/// creates a file to write panic information to, and then we spawn a subprocess
/// that Celeste's GUI get's launched in. Any panics in that subprocess get
/// written to the file, and then they can be handled in the parent process.
///
/// We could have one process and just handle all errors inside of a panic
/// handler there, but I've historically had issues with that. Sadly you can't
/// inspect a panic from inside of a panic handler, so I haven't been able to
/// find the root cause of those issues. I've made the assumption that some
/// library we're using in the GUI during panics is probably in a bad state, so
/// now we just do that in a healthy parent process where there's much less
/// likely to be an issue present.
fn main() {
    // Parse the CLI.
    let cli = Cli::parse();

    // If we're starting up the GUI process, set up the needed panic handler and run
    // the GUI.
    if let Some(path) = cli.panic_file {
        env::set_var("RUST_BACKTRACE", "1");
        panic::set_hook(Box::new(move |panic_info| {
            let backtrace = Backtrace::new();
            let event = PanicIntegration::new().event_from_panic_info(panic_info);
            let json = serde_json::to_string(&SentryPanicInfo { event, backtrace }).unwrap();

            fs::write(&path, json).unwrap();
        }));
        start();
    // Otherwise, start up the parent handler.
    } else {
        parent::start(cli.background);
    }
}
