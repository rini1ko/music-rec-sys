//! Versioned schema changes (R7) on top of `sea-orm-migration`. The applied
//! ones are recorded in `seaql_migrations`. Every migration creates only what
//! is missing, so databases made before migrations existed (by the old
//! `create_schema`) are taken over as they are.

mod m20261011_000001_initial;
mod m20261012_000002_feedback;

use sea_orm::sea_query::{IndexCreateStatement, Table, TableDropStatement};
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, Schema};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261011_000001_initial::Migration),
            Box::new(m20261012_000002_feedback::Migration),
        ]
    }
}

/// Name of every migration and whether it is applied, oldest first.
pub async fn status(db: &DatabaseConnection) -> Result<Vec<(String, bool)>, DbErr> {
    let applied = Migrator::get_applied_migrations(db).await?;
    let pending = Migrator::get_pending_migrations(db).await?;
    Ok(applied
        .iter()
        .map(|migration| (migration.name().to_string(), true))
        .chain(
            pending
                .iter()
                .map(|migration| (migration.name().to_string(), false)),
        )
        .collect())
}

/// Applies the migrations that are not applied yet.
pub async fn up(db: &DatabaseConnection) -> Result<(), DbErr> {
    Migrator::up(db, None).await
}

/// Undoes the last `steps` migrations. It drops tables, so the data in them is lost.
pub async fn down(db: &DatabaseConnection, steps: u32) -> Result<(), DbErr> {
    Migrator::down(db, Some(steps)).await
}

/// A table made from an entity, created only if it is missing.
async fn create_table<E: EntityTrait>(manager: &SchemaManager<'_>, entity: E) -> Result<(), DbErr> {
    let mut statement =
        Schema::new(manager.get_database_backend()).create_table_from_entity(entity);
    statement.if_not_exists();
    manager.create_table(statement).await
}

async fn create_indexes(
    manager: &SchemaManager<'_>,
    indexes: Vec<IndexCreateStatement>,
) -> Result<(), DbErr> {
    for index in indexes {
        manager.create_index(index).await?;
    }
    Ok(())
}

fn drop_table<E: EntityTrait>(entity: E) -> TableDropStatement {
    Table::drop().table(entity).if_exists().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::connect;

    #[tokio::test]
    async fn migrations_run_once_and_can_be_undone() {
        let db = connect("sqlite::memory:").await.unwrap();
        let fresh = status(&db).await.unwrap();
        assert!(fresh.iter().all(|(_, applied)| !applied));

        up(&db).await.unwrap();
        up(&db).await.unwrap();
        let names: Vec<(String, bool)> = status(&db).await.unwrap();
        assert_eq!(names.len(), 2);
        assert!(names.iter().all(|(_, applied)| *applied));

        down(&db, 1).await.unwrap();
        let after = status(&db).await.unwrap();
        assert_eq!(after.iter().filter(|(_, applied)| *applied).count(), 1);
        let manager = SchemaManager::new(&db);
        assert!(!manager.has_table("feedback").await.unwrap());
        assert!(manager.has_table("listen_events").await.unwrap());
    }

    #[tokio::test]
    async fn a_database_made_without_migrations_is_taken_over() {
        let db = connect("sqlite::memory:").await.unwrap();
        // the tables already exist, as in a database made before R7
        let manager = SchemaManager::new(&db);
        create_table(&manager, crate::db::entities::user::Entity)
            .await
            .unwrap();
        create_table(&manager, crate::db::entities::track::Entity)
            .await
            .unwrap();

        up(&db).await.unwrap();
        assert!(
            status(&db)
                .await
                .unwrap()
                .iter()
                .all(|(_, applied)| *applied)
        );
    }
}
