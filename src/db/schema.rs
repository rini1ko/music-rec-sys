use sea_orm::sea_query::{Index, IndexCreateStatement};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbErr, EntityTrait, Schema,
};

use crate::db::entities::{listen_event, session, track, user};

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

/// Creates tables and indexes. Safe to call on an existing database.
pub async fn create_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    // parents first, children reference them
    create_table(db, user::Entity).await?;
    create_table(db, track::Entity).await?;
    create_table(db, listen_event::Entity).await?;
    create_table(db, session::Entity).await?;

    for index in indexes() {
        db.execute(&index).await?;
    }
    Ok(())
}

async fn create_table<E: EntityTrait>(db: &DatabaseConnection, entity: E) -> Result<(), DbErr> {
    let mut statement = Schema::new(db.get_database_backend()).create_table_from_entity(entity);
    statement.if_not_exists();
    db.execute(&statement).await?;
    Ok(())
}

fn indexes() -> Vec<IndexCreateStatement> {
    vec![
        // user_history(): one user's events in time order
        Index::create()
            .if_not_exists()
            .name("idx_events_user_time")
            .table(listen_event::Entity)
            .col(listen_event::Column::UserIndex)
            .col(listen_event::Column::Timestamp)
            .to_owned(),
        // trends: events of the last x days
        Index::create()
            .if_not_exists()
            .name("idx_events_time")
            .table(listen_event::Entity)
            .col(listen_event::Column::Timestamp)
            .to_owned(),
        // popularity of a track, and lookups by foreign key
        Index::create()
            .if_not_exists()
            .name("idx_events_track")
            .table(listen_event::Entity)
            .col(listen_event::Column::TrackIndex)
            .to_owned(),
        // tracks of one artist (artist page)
        Index::create()
            .if_not_exists()
            .name("idx_tracks_artist")
            .table(track::Entity)
            .col(track::Column::ArtistIndex)
            .to_owned(),
        Index::create()
            .if_not_exists()
            .name("idx_sessions_user")
            .table(session::Entity)
            .col(session::Column::UserIndex)
            .to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, PaginatorTrait, Set};

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
