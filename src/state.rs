use std::sync::Arc;
use tokio::sync::mpsc;

use crate::db::Database;
use crate::serial::SerialCommand;

/// Estado compartilhado injetado em todos os handlers pelo Axum.
/// Mantido em Arc para ser clonado de forma barata entre tasks.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    pub db: Database,
    pub jwt_secret: String,
    pub serial_tx: mpsc::Sender<SerialCommand>,
}

impl AppState {
    pub fn new(db: Database, serial_tx: mpsc::Sender<SerialCommand>) -> Self {
        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("JWT_SECRET deve estar definido no .env");

        Self {
            inner: Arc::new(Inner { db, jwt_secret, serial_tx }),
        }
    }

    pub fn db(&self) -> &Database {
        &self.inner.db
    }

    pub fn jwt_secret(&self) -> &str {
        &self.inner.jwt_secret
    }

    // Facilita o acesso ao transmissor da porta serial nas rotas
    pub fn serial_tx(&self) -> mpsc::Sender<SerialCommand> {
        self.inner.serial_tx.clone()
    }
}