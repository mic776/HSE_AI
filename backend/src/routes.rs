use crate::handlers;
use crate::state::AppState;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_credentials(true)
        .allow_origin([
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("https://school-gaming-quiz.ru"),
            HeaderValue::from_static("https://www.school-gaming-quiz.ru"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::COOKIE,
            axum::http::header::SET_COOKIE,
            axum::http::HeaderName::from_static("x-csrf-token"),
            axum::http::HeaderName::from_static("x-request-id"),
            axum::http::HeaderName::from_static("x-forwarded-for"),
        ]);

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/v1/auth/register", post(handlers::register))
        .route("/api/v1/auth/login", post(handlers::login))
        .route("/api/v1/auth/logout", post(handlers::logout))
        .route("/api/v1/auth/me", get(handlers::me))
        .route("/api/v1/quizzes", post(handlers::create_quiz).get(handlers::list_quizzes))
        .route(
            "/api/v1/quizzes/:id",
            get(handlers::get_quiz).put(handlers::update_quiz).delete(handlers::delete_quiz),
        )
        .route("/api/v1/quizzes/:id/publish", post(handlers::publish_quiz))
        .route("/api/v1/quizzes/:id/unpublish", post(handlers::unpublish_quiz))
        .route("/api/v1/quizzes/:id/clone", post(handlers::clone_quiz))
        .route("/api/v1/library/quizzes", get(handlers::library_list))
        .route("/api/v1/ai/generate-quiz", post(handlers::ai_generate_quiz))
        .route("/api/v1/sessions", post(handlers::create_session))
        .route("/api/v1/sessions/:id/start", post(handlers::start_session))
        .route("/api/v1/sessions/:id/end", post(handlers::end_session))
        .route("/api/v1/sessions/:id/results", get(handlers::session_results))
        .route("/ws/sessions/:room_code", get(handlers::ws_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}
