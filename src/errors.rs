use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Erros da aplicação mapeados para respostas HTTP.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Não autorizado: {0}")]
    Unauthorized(String),

    #[error("Não encontrado: {0}")]
    NotFound(String),

    #[error("Requisição inválida: {0}")]
    BadRequest(String),

    #[error("Acesso proibido")]
    Forbidden,

    #[error("Erro interno: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("Erro de banco de dados: {0}")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Acesso proibido".to_string()),
            AppError::Internal(e) => {
                tracing::error!("Erro interno: {e:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Erro interno do servidor".to_string())
            }
            AppError::Database(e) => {
                tracing::error!("Erro de banco: {e:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Erro de banco de dados".to_string())
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
