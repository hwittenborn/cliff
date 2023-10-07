//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.3
use crate::util;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_dirs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub remote_id: i32,
    /// The local directory being synced, as an absolute path with no '/' at the
    /// end.
    pub local_path: String,
    /// The remote path being synced, as an absolute path (though it won't start
    /// with `/`).
    pub remote_path: String,
}

impl Model {
    // See if this item still exists in the database (i.e. the struct was created
    // and the item was later deleted).
    pub fn exists(&self, db: &DatabaseConnection) -> bool {
        util::await_future(
            Entity::find()
                .filter(Column::LocalPath.eq(self.local_path.clone()))
                .filter(Column::RemotePath.eq(self.remote_path.clone()))
                .one(db),
        )
        .unwrap()
        .is_some()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::remotes::Entity",
        from = "Column::RemoteId",
        to = "super::remotes::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Remotes,
    #[sea_orm(has_many = "super::sync_items::Entity")]
    SyncItems,
}

impl Related<super::remotes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Remotes.def()
    }
}

impl Related<super::sync_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}