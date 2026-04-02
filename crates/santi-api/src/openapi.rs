use utoipa::OpenApi;

use crate::handler::{admin, health, meta, session};
use crate::schema::{
    admin as admin_schema,
    common::ErrorResponse,
    health::HealthResponse,
    meta::MetaResponse,
    session::{
        SessionCompactRequest, SessionCompactResponse, SessionEffectsResponse,
        SessionMemoryRequest, SessionMemoryResponse, SessionMessagesResponse, SessionResponse,
        SessionSendRequest, SoulMemoryRequest, SoulMemoryResponse, SoulResponse,
    },
    soul::{
        SoulMemoryRequest as LocalSoulMemoryRequest, SoulMemoryResponse as LocalSoulMemoryResponse,
        SoulResponse as LocalSoulResponse,
    },
};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health,
        meta::meta,
        admin::reload_hooks,
        session::get_default_soul,
        session::set_default_soul_memory,
        session::create_session,
        session::get_session,
        session::send_session,
        session::fork_session,
        session::compact_session,
        session::get_session_memory,
        session::set_session_memory,
        session::list_session_messages,
        session::list_session_effects,
        session::list_session_compacts,
    ),
    components(schemas(
        HealthResponse,
        MetaResponse,
        ErrorResponse,
        admin_schema::HookReloadRequest,
        admin_schema::HookReloadResponse,
        SessionResponse,
        SessionMemoryRequest,
        SessionMemoryResponse,
        SoulResponse,
        SoulMemoryRequest,
        SoulMemoryResponse,
        LocalSoulResponse,
        LocalSoulMemoryRequest,
        LocalSoulMemoryResponse,
        SessionSendRequest,
        SessionCompactRequest,
        SessionCompactResponse,
        SessionMessagesResponse,
        SessionEffectsResponse,
    )),
    tags(
        (name = "health"),
        (name = "meta"),
        (name = "admin"),
        (name = "soul"),
        (name = "session"),
    )
)]
pub struct ApiDoc;
