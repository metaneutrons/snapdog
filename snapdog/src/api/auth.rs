// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Optional API key authentication middleware.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Middleware that checks `Authorization: Bearer <key>` on API routes.
pub async fn require_api_key(request: Request, next: Next) -> Response {
    let key = request.extensions().get::<ApiKey>().map(|k| k.0.as_str());

    let Some(expected) = key else {
        // No API key configured — pass through
        return next.run(request).await;
    };

    // Check Authorization header
    let auth = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let authorized = match auth {
        Some(v) if v.starts_with("Bearer ") => &v[7..] == expected,
        _ => {
            // Check query param ?token= (for WebSocket)
            request
                .uri()
                .query()
                .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("token=")))
                .is_some_and(|t| t == expected)
        }
    };

    if authorized {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid or missing API key").into_response()
    }
}

/// Extension type to pass the API key into the middleware.
#[derive(Clone)]
pub struct ApiKey(pub String);
