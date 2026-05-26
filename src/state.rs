use std::sync::Arc;

use crate::db::Database;

/// Estado compartilhado injetado em todos os handlers pelo Axum.
/// Mantido em Arc para ser clonado de forma barata entre tasks.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    pub db: Database,
    pub jwt_secret: String,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("JWT_SECRET deve estar definido no .env");

        Self {
            inner: Arc::new(Inner { db, jwt_secret }),
        }
    }

    pub fn db(&self) -> &Database {
        &self.inner.db
    }

    pub fn jwt_secret(&self) -> &str {
        &self.inner.jwt_secret
    }
}
