//! Likes and skips (R3).

use sea_orm::DbErr;
use sea_orm::sea_query::Index;
use sea_orm_migration::{MigrationTrait, SchemaManager, async_trait, prelude::DeriveMigrationName};

use super::{create_indexes, create_table, drop_table};
use crate::db::entities::feedback;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_table(manager, feedback::Entity).await?;
        // a user's feedback, newest first, and the sums per model
        let by_user = Index::create()
            .if_not_exists()
            .name("idx_feedback_user_time")
            .table(feedback::Entity)
            .col(feedback::Column::UserIndex)
            .col(feedback::Column::CreatedAt)
            .to_owned();
        create_indexes(manager, vec![by_user]).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(drop_table(feedback::Entity)).await
    }
}
