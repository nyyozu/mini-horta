use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
    RequestPartsExt,
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{errors::AppError, models::UserRole, state::AppState};

// ── Claims JWT ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,        // user_id
    pub role: UserRole,
    pub exp: i64,         // Unix timestamp de expiração
    pub iat: i64,         // Unix timestamp de emissão
}

/// Gera um token JWT válido por 24 horas.
pub fn create_token(user_id: Uuid, role: UserRole, secret: &str) -> anyhow::Result<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        role,
        iat: now.timestamp(),
        exp: (now + Duration::hours(24)).timestamp(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

/// Valida e decodifica um token JWT.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(format!("Token inválido: {e}")))?;

    Ok(data.claims)
}

// ── Extractors Axum ────────────────────────────────────────────────────────────

/// Extrator para qualquer usuário autenticado.
/// Use `AuthUser` como parâmetro de handler para proteger a rota.
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::Unauthorized("Token ausente".to_string()))?;

        let claims = verify_token(bearer.token(), state.jwt_secret())?;
        Ok(AuthUser(claims))
    }
}

/// Extrator exclusivo para administradores.
/// Retorna 403 se o usuário autenticado não for admin.
#[derive(Debug, Clone)]
pub struct AdminUser(pub Claims);

#[async_trait]
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;

        if claims.role != UserRole::Admin {
            return Err(AppError::Forbidden);
        }

        Ok(AdminUser(claims))
    }
}