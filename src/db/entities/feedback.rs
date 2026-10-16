use sea_orm::entity::prelude::*;

/// A like or a skip of a track (R3).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "feedback")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_index: u32,
    pub track_index: u32,
    /// `like` or `skip`.
    pub kind: String,
    /// The model the track was credited to, `None` if no model proposed it.
    pub source: Option<String>,
    pub created_at: DateTime,
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
