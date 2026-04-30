use crate::schema::common::ErrorResponse;

#[derive(Clone, Debug)]
pub enum ApiError {
    NotFound(String),
    Conflict(String),
    Validation(String),
    Unsupported(String),
    BadRequest(String),
    Internal(String),
}

impl ApiError {
    pub fn into_error_response(self) -> (axum::http::StatusCode, axum::Json<ErrorResponse>) {
        match self {
            Self::NotFound(message) => (
                axum::http::StatusCode::NOT_FOUND,
                axum::Json(ErrorResponse::new("not_found", message)),
            ),
            Self::Conflict(message) => (
                axum::http::StatusCode::CONFLICT,
                axum::Json(ErrorResponse::new("conflict", message)),
            ),
            Self::Validation(message) => (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse::new("validation_error", message)),
            ),
            Self::Unsupported(message) => (
                axum::http::StatusCode::NOT_IMPLEMENTED,
                axum::Json(ErrorResponse::new("not_implemented", message)),
            ),
            Self::BadRequest(message) => (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse::new("bad_request", message)),
            ),
            Self::Internal(message) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse::new("internal_error", message)),
            ),
        }
    }
}
