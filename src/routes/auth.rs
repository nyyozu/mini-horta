use axum::{Json, extract::State};
use bcrypt::{DEFAULT_COST, hash, verify};

use crate::{
    auth::create_token,
    errors::{AppError, AppResult},
    models::{CreateUserRequest, LoginRequest, LoginResponse, UserRole},
    state::AppState,
};

/// POST /auth/register
/// Cria um novo usuário (role padrão = User).
/// Apenas admins deveriam chamar com role=admin — valide no front ou adicione
/// o extrator AdminUser se quiser restringir no servidor.
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Validação básica de email
    if req.email.is_empty() || !req.email.contains('@') {
        return Err(AppError::BadRequest("E-mail inválido".to_string()));
    }
    if req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Senha deve ter pelo menos 8 caracteres".to_string(),
        ));
    }

    let password_hash = hash(&req.password, DEFAULT_COST)
        .map_err(|e| AppError::Internal(e.into()))?;

    let role = req.role.unwrap_or(UserRole::User);

    let user = state
        .db()
        .create_user(&req.email, &password_hash, &role)
        .await
        // FIX: db() agora retorna anyhow::Result, não sqlx::Result.
        // Inspecionamos a mensagem para detectar UNIQUE antes de converter
        // para AppError::Internal (que aceita anyhow::Error via #[from]).
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                AppError::BadRequest("E-mail já cadastrado".to_string())
            } else {
                AppError::Internal(e)
            }
        })?;

    Ok(Json(serde_json::json!({
        "id": user.id,
        "email": user.email,
        "role": user.role,
    })))
}

/// POST /auth/login
/// Autentica e retorna um JWT.
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let user = state
        .db()
        .find_user_by_email(&req.email)
        .await
        // FIX: mesma razão — converte anyhow::Error → AppError::Internal
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::Unauthorized("Credenciais inválidas".to_string()))?;

    let valid = verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::Unauthorized("Credenciais inválidas".to_string()))?;

    if !valid {
        return Err(AppError::Unauthorized("Credenciais inválidas".to_string()));
    }

    let token = create_token(user.id, user.role.clone(), state.jwt_secret())
        .map_err(AppError::Internal)?;

    Ok(Json(LoginResponse {
        token,
        user_id: user.id,
        role: user.role,
    }))
}