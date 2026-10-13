use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

use crate::db::migration;

/// `url` examples: `sqlite://data/music.db?mode=rwc` (file, created if missing)
/// or `sqlite::memory:` (tests).
///
/// One connection only: every in-memory connection would be its own empty
/// database, and SQLite allows a single writer anyway.
pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut options = ConnectOptions::new(url.to_owned());
    options.max_connections(1).sqlx_logging(false);
    Database::connect(options).await
}

/// Brings the schema up to date by applying the pending migrations (R7).
/// Safe to call on an existing database, also one made before migrations.
pub async fn create_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    migration::up(db).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::{listen_event, user};
    use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};

    #[tokio::test]
    async fn schema_can_be_created_twice() {
        let db = connect("sqlite::memory:").await.unwrap();
        create_schema(&db).await.unwrap();
        create_schema(&db).await.unwrap();

        assert_eq!(user::Entity::find().count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced() {
        let db = connect("sqlite::memory:").await.unwrap();
        create_schema(&db).await.unwrap();

        let orphan = listen_event::ActiveModel {
            user_index: Set(1),
            track_index: Set(1),
            timestamp: Set(chrono::NaiveDate::from_ymd_opt(2009, 5, 4)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap()),
            ..Default::default()
        };
        assert!(orphan.insert(&db).await.is_err());
    }
}
