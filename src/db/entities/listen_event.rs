use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "listen_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_index: u32,
    pub track_index: u32,
    pub timestamp: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserIndex",
        to = "super::user::Column::UserIndex",
        on_delete = "Cascade"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::track::Entity",
        from = "Column::TrackIndex",
        to = "super::track::Column::TrackIndex"
    )]
    Track,
}

impl ActiveModelBehavior for ActiveModel {}
