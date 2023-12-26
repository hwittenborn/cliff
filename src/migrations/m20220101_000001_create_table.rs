use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE remotes (
                id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
                name TEXT NOT NULL
            );
    
    
            CREATE TABLE sync_dirs (
                id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
                remote_id INTEGER NOT NULL,
                local_path TEXT NOT NULL,
                remote_path TEXT NOT NULL,
                FOREIGN KEY(remote_id) REFERENCES remotes(id)
            );
    
            CREATE TABLE sync_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
                sync_dir_id INTEGER NOT NULL,
                local_path TEXT NOT NULL,
                remote_path TEXT NOT NULL,
                last_local_timestamp INTEGER NOT NULL,
                last_remote_timestamp INTEGER NOT NULL,
                FOREIGN KEY(sync_dir_id) REFERENCES sync_dirs(id)
            );
        "#;
        let stmt = Statement::from_string(manager.get_database_backend(), sql.to_owned());
        manager.get_connection().execute(stmt).await.map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = "
            DROP TABLE `sync_items`;
            DROP TABLE `sync_dirs`;
            DROP TABLE `remotes`;
        ";
        let stmt = Statement::from_string(manager.get_database_backend(), sql.to_owned());
        manager.get_connection().execute(stmt).await.map(|_| ())
    }
}
