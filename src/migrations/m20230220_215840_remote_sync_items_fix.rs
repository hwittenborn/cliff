use if_chain::if_chain;
use sea_orm::{ConnectionTrait, FromQueryResult, JsonValue, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        let sync_items = JsonValue::find_by_statement(Statement::from_sql_and_values(
            backend,
            "SELECT remote_path FROM sync_items;",
            [],
        ))
        .all(db)
        .await?;

        for item in sync_items {
            if_chain! {
                if let Some(item_str) = item["remote_path"].as_str();
                if item_str.starts_with('/');
                then {
                    db.execute(Statement::from_string(
                            backend,
                            format!(r#"DELETE FROM sync_items where remote_path = "{item_str}""#)
                        ))
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
