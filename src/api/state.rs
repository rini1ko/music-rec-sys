use crate::db::store::Store;

/// Shared by every request. Cloning is cheap, the pool lives behind the store.
#[derive(Clone)]
pub struct AppState {
    pub store: Store,
}