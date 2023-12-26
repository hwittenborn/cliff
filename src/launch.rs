use std::{
    convert::identity,
    fs::{self, File},
    path::Path,
};

use adw::prelude::*;
use futures::executor::BlockAwait;
use relm4::{
    component::{
        AsyncComponent, AsyncComponentController, AsyncComponentParts, AsyncComponentSender,
        AsyncController, SimpleAsyncComponent,
    },
    prelude::*,
};
use relm4_components::alert::{Alert, AlertMsg, AlertSettings};
use sea_orm::{Database, DatabaseConnection};
use tokio::sync::mpsc;

use crate::{
    login::{LoginModel, LoginMsg, LoginOutput},
    migrations::{Migrator, MigratorTrait},
    util,
};

#[derive(Debug)]
pub enum LaunchMsg {
    /// The user is trying to add a new remote.
    AddLogin,
    /// The application window is trying to be opened (i.e. from the tray).
    OpenRequest,
    /// The application is trying to be closed (i.e. from the tray).
    CloseRequest,
    #[doc(hidden)]
    /// A new server has been added (from a succesful login).
    NewLogin(String),
    /// A new login has been cancelled.
    LoginCancel,
}

pub struct LaunchModel {
    /// Whether the window should hide on close, or just close. We only hide on
    /// close when the main syncing window is open (i.e. make it close normally
    /// when logging in).
    hide_on_close: bool,
    /// Whether the main application window should be visible. We hide it when
    /// there's no remotes and we need to log in to a new one.
    visible: bool,
    /// The login window.
    login: AsyncController<LoginModel>,
    // /// The connection to Celeste's database.
    // db: DatabaseConnection,
}

#[relm4::component(async, pub)]
impl SimpleAsyncComponent for LaunchModel {
    type Input = LaunchMsg;
    type Output = ();
    // type Init = DatabaseConnection;
    type Init = ();

    view! {
        adw::ApplicationWindow {
            #[watch]
            set_hide_on_close: model.hide_on_close,
            #[watch]
            set_visible: model.visible,

            adw::HeaderBar {}
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        panic!("Sentry panic lets GOOOOOOOOO");
        let login = LoginModel::builder()
            .transient_for(root.clone())
            .launch(())
            .forward(sender.input_sender(), |resp| match resp {
                LoginOutput::NewLogin(remote_name) => LaunchMsg::NewLogin(remote_name),
                LoginOutput::LoginCancel => LaunchMsg::LoginCancel,
            });
        let model = Self {
            hide_on_close: false,
            visible: false,
            login,
            // db,
        };

        let widgets = view_output!();

        // model.login.emit(LoginMsg::Open);
        AsyncComponentParts { model, widgets }
    }
}
