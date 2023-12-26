use sea_orm::{ConnectionTrait, FromQueryResult, JsonValue, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let local_dirs = JsonValue::find_by_statement(Statement::from_sql_and_values(
            manager.get_database_backend(),
            r#"select local_path from sync_dirs"#,
            [],
        ))
        .all(manager.get_connection())
        .await?;

        for dir in local_dirs {
            let dir_string = match dir["local_path"].as_str().unwrap().strip_suffix('/') {
                Some(string) => string,
                None => continue,
            };

            manager.get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    format!(r#"update sync_dirs set local_path = "{dir_string}" where local_path = "{dir_string}/""#)
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
