use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("autenticação necessária")]
    Unauthorized,
    #[error("sem permissão para esta ação")]
    Forbidden,
    #[error("não encontrado")]
    NotFound,
    #[error("{message}")]
    Conflict { message: String, details: serde_json::Value },
    #[error("erro interno")]
    Internal(#[from] eyre::Report),
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn bad(message: impl Into<String>) -> Self {
        ApiError::BadRequest(message.into())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::RowNotFound => ApiError::NotFound,
            sqlx::Error::Database(db) if db.is_unique_violation() => ApiError::Conflict {
                message: "registro já existe".into(),
                details: json!({ "constraint": db.constraint() }),
            },
            other => ApiError::Internal(other.into()),
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(error: std::io::Error) -> Self {
        ApiError::Internal(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Conflict { .. } => (StatusCode::CONFLICT, "conflict"),
            ApiError::Internal(error) => {
                tracing::error!("erro interno: {error:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        let details = match &self {
            ApiError::Conflict { details, .. } => details.clone(),
            _ => serde_json::Value::Null,
        };
        (status, Json(json!({ "error": code, "message": self.to_string(), "details": details }))).into_response()
    }
}
