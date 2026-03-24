use utoipa::OpenApi;

use crate::{
    handler,
    schema::{
        common::ErrorResponse,
        health::HealthResponse,
        session::{
            SessionMemoryRequest, SessionMemoryResponse, SessionMessagesResponse, SessionResponse,
            SoulMemoryRequest, SoulMemoryResponse, SoulResponse,
        },
    },
};

#[derive(OpenApi)]
#[openapi(
    paths(
        handler::health::health,
        handler::session::get_default_soul,
        handler::session::set_default_soul_memory,
        handler::session::create_session,
        handler::session::get_session,
        handler::session::send_session,
        handler::session::list_session_messages,
        handler::session::set_session_memory
    ),
    components(schemas(
        ErrorResponse,
        HealthResponse,
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
        (name = "health", description = "Health check endpoints"),
        (name = "session", description = "Session runtime endpoints"),
        (name = "soul", description = "Soul runtime endpoints")
    )
)]
pub struct ApiDoc;
