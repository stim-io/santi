use utoipa::OpenApi;

use crate::{
    handler,
    schema::{
        admin::{HookReloadRequest, HookReloadResponse},
        common::ErrorResponse,
        health::HealthResponse,
        session::{
            ForkRequest, ForkResponse, SessionCompactRequest, SessionCompactResponse,
            SessionEffectResponse, SessionEffectsResponse, SessionMemoryRequest,
            SessionMemoryResponse, SessionMessagesResponse, SessionResponse, SoulMemoryRequest,
            SoulMemoryResponse, SoulResponse,
        },
    },
};

#[derive(OpenApi)]
#[openapi(
    paths(
        handler::health::health,
        handler::admin::reload_hooks,
        handler::session::get_default_soul,
        handler::session::set_default_soul_memory,
        handler::session::create_session,
        handler::session::get_session,
        handler::session::send_session,
        handler::session::list_session_messages,
        handler::session::list_session_effects,
        handler::session::fork_session,
        handler::session::compact_session,
        handler::session::set_session_memory
    ),
    components(schemas(
        ErrorResponse,
        ForkRequest,
        ForkResponse,
        HookReloadRequest,
        HookReloadResponse,
        HealthResponse,
        SessionCompactRequest,
        SessionCompactResponse,
        SessionEffectResponse,
        SessionEffectsResponse,
        SessionMemoryRequest,
        SessionMemoryResponse,
        crate::schema::session::SessionSendRequest,
        SessionResponse,
        SessionMessagesResponse,
        SoulMemoryRequest,
        SoulMemoryResponse,
        SoulResponse
    )),
    tags(
        (name = "admin", description = "Admin management endpoints"),
        (name = "health", description = "Health check endpoints"),
        (name = "session", description = "Session runtime endpoints"),
        (name = "soul", description = "Soul runtime endpoints")
    )
)]
pub struct ApiDoc;
