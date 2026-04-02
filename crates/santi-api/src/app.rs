use axum::{
    routing::{get, post, put},
    Router,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{handler, openapi::ApiDoc, state::AppState};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(handler::health::health))
        .route("/api/v1/meta", get(handler::meta::meta))
        .route("/api/v1/admin/hooks", put(handler::admin::reload_hooks))
        .route("/api/v1/soul", get(handler::session::get_default_soul))
        .route(
            "/api/v1/soul/memory",
            put(handler::session::set_default_soul_memory),
        )
        .route("/api/v1/sessions", post(handler::session::create_session))
        .route("/api/v1/sessions/:id", get(handler::session::get_session))
        .route(
            "/api/v1/sessions/:id/send",
            post(handler::session::send_session),
        )
        .route(
            "/api/v1/sessions/:id/fork",
            post(handler::session::fork_session),
        )
        .route(
            "/api/v1/sessions/:id/compact",
            post(handler::session::compact_session),
        )
        .route(
            "/api/v1/sessions/:id/memory",
            get(handler::session::get_session_memory).put(handler::session::set_session_memory),
        )
        .route(
            "/api/v1/sessions/:id/messages",
            get(handler::session::list_session_messages),
        )
        .route(
            "/api/v1/sessions/:id/effects",
            get(handler::session::list_session_effects),
        )
        .route(
            "/api/v1/sessions/:id/compacts",
            get(handler::session::list_session_compacts),
        )
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}
