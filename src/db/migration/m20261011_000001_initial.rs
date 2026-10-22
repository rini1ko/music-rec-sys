//! Users, tracks, plays and sessions with the indexes the queries rely on.

use sea_orm::DbErr;
use sea_orm::sea_query::{Index, IndexCreateStatement};
use sea_orm_migration::{MigrationTrait, SchemaManager, async_trait, prelude::DeriveMigrationName};

use super::{create_indexes, create_table, drop_table};
use crate::db::entities::{listen_event, session, track, user};

#[derive(DeriveMigrationName)]
pub struct Migration;

fn indexes() -> Vec<IndexCreateStatement> {
    vec![
        // one user's events in time order (history, recent plays)
        Index::create()
            .if_not_exists()
            .name("idx_events_user_time")
            .table(listen_event::Entity)
            .col(listen_event::Column::UserIndex)
            .col(listen_event::Column::Timestamp)
            .to_owned(),
        // trends and statistics: events of the last days
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
        // tracks of one artist
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

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // parents first, children reference them
        create_table(manager, user::Entity).await?;
        create_table(manager, track::Entity).await?;
        create_table(manager, listen_event::Entity).await?;
        create_table(manager, session::Entity).await?;
        create_indexes(manager, indexes()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // children first
        manager.drop_table(drop_table(session::Entity)).await?;
        manager.drop_table(drop_table(listen_event::Entity)).await?;
        manager.drop_table(drop_table(track::Entity)).await?;
        manager.drop_table(drop_table(user::Entity)).await
    }
}
