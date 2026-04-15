// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Typed API error responses.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

/// API error with JSON body.
pub enum ApiError {
    /// Resource not found (404).
    NotFound(&'static str),
    /// Invalid request parameters (400).
    BadRequest(String),
    /// Request understood but cannot be processed (422).
    Unprocessable(String),
    /// Required service not configured (503).
    ServiceUnavailable(&'static str),
    /// Upstream service error (502).
    BadGateway(String),
    /// Unexpected internal error (500).
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            Self::NotFound(what) => (
                StatusCode::NOT_FOUND,
                "not_found",
                format!("{what} not found"),
            ),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            Self::Unprocessable(msg) => (StatusCode::UNPROCESSABLE_ENTITY, "unprocessable", msg),
            Self::ServiceUnavailable(what) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                format!("{what} not configured"),
            ),
            Self::BadGateway(msg) => (StatusCode::BAD_GATEWAY, "bad_gateway", msg),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", msg),
        };
        (status, Json(ErrorBody { error, message })).into_response()
    }
}
