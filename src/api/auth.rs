use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use crate::api::AppState;

#[allow(dead_code)]
pub struct AuthUser {
    pub key_id: Option<String>,
    pub key_name: Option<String>,
}

#[allow(dead_code)]
pub struct RequireAuth(pub AuthUser);

impl FromRequestParts<AppState> for RequireAuth {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        if !state.config.auth.enabled {
            return Ok(RequireAuth(AuthUser {
                key_id: None,
                key_name: Some("anonymous".to_string()),
            }));
        }

        // Try Authorization: Bearer <key>
        let mut raw_token = None;
        if let Some(auth_hdr) = parts.headers.get("Authorization") {
            if let Ok(hdr_str) = auth_hdr.to_str() {
                if hdr_str.starts_with("Bearer ") {
                    raw_token = Some(hdr_str[7..].trim().to_string());
                }
            }
        }

        // Try x-api-key: <key>
        if raw_token.is_none() {
            if let Some(api_hdr) = parts.headers.get("x-api-key") {
                if let Ok(hdr_str) = api_hdr.to_str() {
                    raw_token = Some(hdr_str.trim().to_string());
                }
            }
        }

        if let Some(token) = raw_token {
            match state.db.verify_api_key(&token) {
                Ok(Some(record)) => Ok(RequireAuth(AuthUser {
                    key_id: Some(record.id),
                    key_name: Some(record.name),
                })),
                _ => Err((
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({
                        "error": {
                            "message": "Invalid or revoked API key",
                            "type": "invalid_request_error",
                            "code": "invalid_api_key"
                        }
                    })),
                ).into_response()),
            }
        } else {
            Err((
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({
                    "error": {
                        "message": "Missing API key in Authorization header or x-api-key",
                        "type": "invalid_request_error",
                        "code": "missing_api_key"
                    }
                })),
            ).into_response())
        }
    }
}
