use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response, Sse},
    Json,
};
use futures::StreamExt;
use std::convert::Infallible;

use crate::{
    handler::session_events::{done_event, encode_session_sse_event},
    schema::{
        common::ErrorResponse,
        session::{
            ForkRequest, ForkResponse, SessionCompactRequest, SessionCompactResponse,
            SessionCompactsResponse, SessionEffectsResponse, SessionMemoryRequest,
            SessionMemoryResponse, SessionMessagesResponse, SessionResponse,
            SessionSendContentPart, SessionSendRequest, SoulMemoryRequest, SoulMemoryResponse,
            SoulResponse,
        },
        session_events::SessionStreamEvent,
    },
    state::AppState,
};

#[utoipa::path(
    post,
    path = "/api/v1/sessions",
    tag = "session",
    responses(
        (status = 201, description = "Session created", body = SessionResponse)
    )
)]
pub async fn create_session(State(state): State<AppState>) -> impl IntoResponse {
    match state.session_api().create_session().await {
        Ok(session) => (StatusCode::CREATED, Json(SessionResponse::from(session))).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sessions/{id}",
    tag = "session",
    params(
        ("id" = String, Path, description = "Session id")
    ),
    responses(
        (status = 200, description = "Session found", body = SessionResponse),
        (status = 404, description = "Session not found", body = ErrorResponse)
    )
)]
pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.session_api().get_session(&id).await {
        Ok(session) => (StatusCode::OK, Json(SessionResponse::from(session))).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sessions/{id}/messages",
    tag = "session",
    params(
        ("id" = String, Path, description = "Session id")
    ),
    responses(
        (status = 200, description = "Session messages", body = SessionMessagesResponse),
        (status = 404, description = "Session not found", body = ErrorResponse)
    )
)]
pub async fn list_session_messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.session_api().list_session_messages(&id).await {
        Ok(messages) => (
            StatusCode::OK,
            Json(SessionMessagesResponse::from_messages(messages)),
        )
            .into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sessions/{id}/effects",
    tag = "session",
    params(
        ("id" = String, Path, description = "Session id")
    ),
    responses(
        (status = 200, description = "Session effects", body = SessionEffectsResponse),
        (status = 404, description = "Session not found", body = ErrorResponse)
    )
)]
pub async fn list_session_effects(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.session_api().list_session_effects(&id).await {
        Ok(effects) => (
            StatusCode::OK,
            Json(SessionEffectsResponse::from_effects(effects)),
        )
            .into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sessions/{id}/compacts",
    tag = "session",
    params(
        ("id" = String, Path, description = "Session id")
    ),
    responses(
        (status = 200, description = "Session compacts", body = SessionCompactsResponse),
        (status = 404, description = "Session not found", body = ErrorResponse)
    )
)]
pub async fn list_session_compacts(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.session_api().list_session_compacts(&id).await {
        Ok(compacts) => (
            StatusCode::OK,
            Json(SessionCompactsResponse::from_compacts(compacts)),
        )
            .into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sessions/{id}/memory",
    tag = "session",
    params(
        ("id" = String, Path, description = "Session id")
    ),
    responses(
        (status = 200, description = "Session memory found", body = SessionMemoryResponse),
        (status = 404, description = "Session not found", body = ErrorResponse)
    )
)]
pub async fn get_session_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.session_api().get_session_memory(&id).await {
        Ok(memory) => (StatusCode::OK, Json(memory)).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sessions/{id}/memory",
    tag = "session",
    params(
        ("id" = String, Path, description = "Session id")
    ),
    request_body(content = SessionMemoryRequest),
    responses(
        (status = 200, description = "Session memory updated", body = SessionMemoryResponse),
        (status = 404, description = "Session not found", body = ErrorResponse)
    )
)]
pub async fn set_session_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<SessionMemoryRequest>,
) -> impl IntoResponse {
    match state
        .session_api()
        .set_session_memory(&id, &request.text)
        .await
    {
        Ok(memory) => (StatusCode::OK, Json(memory)).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/soul",
    tag = "soul",
    responses(
        (status = 200, description = "Default soul", body = SoulResponse),
        (status = 404, description = "Soul not found", body = ErrorResponse)
    )
)]
pub async fn get_default_soul(State(state): State<AppState>) -> impl IntoResponse {
    match state.soul_api().get_default_soul().await {
        Ok(soul) => (StatusCode::OK, Json(SoulResponse::from(soul))).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/soul/memory",
    tag = "soul",
    request_body(content = SoulMemoryRequest),
    responses(
        (status = 200, description = "Soul memory updated", body = SoulMemoryResponse),
        (status = 404, description = "Soul not found", body = ErrorResponse)
    )
)]
pub async fn set_default_soul_memory(
    State(state): State<AppState>,
    Json(request): Json<SoulMemoryRequest>,
) -> impl IntoResponse {
    match state
        .soul_api()
        .set_default_soul_memory(&request.text)
        .await
    {
        Ok(soul) => (StatusCode::OK, Json(SoulMemoryResponse::from(soul))).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sessions/{id}/send",
    tag = "session",
    params(
        ("id" = String, Path, description = "Session id")
    ),
    request_body(content = SessionSendRequest),
    responses(
        (status = 200, description = "Session send response"),
        (status = 404, description = "Session not found", body = ErrorResponse)
    )
)]
pub async fn send_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<SessionSendRequest>,
) -> Response {
    let user_content = request
        .content
        .into_iter()
        .map(|part| match part {
            SessionSendContentPart::Text { text } => text,
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let stream = match state.session_api().send_session(&id, user_content).await {
        Ok(stream) => stream,
        Err(err) => return err.into_error_response().into_response(),
    };

    let stream = stream
        .map(move |result| match result {
            Ok(event) => Ok::<_, Infallible>(match event {
                SessionStreamEvent::OutputTextDelta(text) => {
                    encode_session_sse_event(SessionStreamEvent::OutputTextDelta(text))
                }
                SessionStreamEvent::Completed => {
                    encode_session_sse_event(SessionStreamEvent::Completed)
                }
            }),
            Err(err) => Ok::<_, Infallible>(axum::response::sse::Event::default().data(
                serde_json::to_string(&err.into_error_response().1 .0).unwrap_or_else(|_| {
                    "{\"error\":{\"code\":\"internal_error\",\"message\":\"internal error\"}}"
                        .to_string()
                }),
            )),
        })
        .chain(futures::stream::once(async {
            Ok::<_, Infallible>(done_event())
        }));

    Sse::new(stream).into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/sessions/{id}/fork",
    tag = "session",
    params(
        ("id" = String, Path, description = "Parent session id")
    ),
    request_body(content = ForkRequest),
    responses(
        (status = 201, description = "Forked session created", body = ForkResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Parent session not found", body = ErrorResponse),
        (status = 409, description = "Session busy", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse)
    )
)]
pub async fn fork_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ForkRequest>,
) -> impl IntoResponse {
    match state
        .session_api()
        .fork_session(&id, req.fork_point, req.request_id)
        .await
    {
        Ok(res) => (StatusCode::CREATED, Json(ForkResponse::from_result(res))).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sessions/{id}/compact",
    tag = "session",
    params(
        ("id" = String, Path, description = "Session id")
    ),
    request_body(content = SessionCompactRequest),
    responses(
        (status = 200, description = "Session compact created", body = SessionCompactResponse),
        (status = 404, description = "Session not found", body = ErrorResponse)
    )
)]
pub async fn compact_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<SessionCompactRequest>,
) -> impl IntoResponse {
    match state
        .session_api()
        .compact_session(&id, &request.summary)
        .await
    {
        Ok(compact) => {
            (StatusCode::OK, Json(SessionCompactResponse::from(compact))).into_response()
        }
        Err(err) => err.into_error_response().into_response(),
    }
}

impl ForkResponse {
    fn from_result(value: santi_runtime::session::fork::ForkResult) -> Self {
        Self {
            new_session_id: value.new_session_id,
            parent_session_id: value.parent_session_id,
            fork_point: value.fork_point,
        }
    }
}
