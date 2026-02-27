use crate::error::{AppError, ErrorDetail};
use crate::models::{score_answer, validate_quiz, Quiz, StudentStats, SubmittedAnswer};
use crate::state::{AppState, ParticipantState, QuizRecord, SessionRecord, Teacher, TeacherSession};
use crate::ws_protocol::WsEnvelope;
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{info, warn};
use once_cell::sync::Lazy;
use dashmap::DashMap;

const SESSION_COOKIE: &str = "teacher_session";
static RATE_LIMIT: Lazy<DashMap<String, (u32, Instant)>> = Lazy::new(DashMap::new);

fn check_rate_limit(scope: &str, key: &str, limit_per_minute: u32) -> bool {
    let now = Instant::now();
    let full_key = format!("{scope}:{key}");
    if let Some(mut entry) = RATE_LIMIT.get_mut(&full_key) {
        if now.duration_since(entry.1) > Duration::from_secs(60) {
            *entry = (1, now);
            true
        } else if entry.0 >= limit_per_minute {
            false
        } else {
            entry.0 += 1;
            true
        }
    } else {
        RATE_LIMIT.insert(full_key, (1, now));
        true
    }
}

fn request_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

async fn auth_teacher_id(jar: &CookieJar, state: &AppState) -> Option<i64> {
    let sid = jar.get(SESSION_COOKIE)?.value().to_string();
    let sessions = state.db.sessions.read().await;
    sessions.get(&sid).map(|v| v.teacher_id)
}

async fn ensure_csrf(headers: &HeaderMap, jar: &CookieJar, state: &AppState) -> bool {
    let sid = match jar.get(SESSION_COOKIE) {
        Some(v) => v.value().to_string(),
        None => return false,
    };
    let header = match headers.get("x-csrf-token").and_then(|h| h.to_str().ok()) {
        Some(v) => v,
        None => return false,
    };
    let sessions = state.db.sessions.read().await;
    sessions
        .get(&sid)
        .map(|s| s.csrf_token == header)
        .unwrap_or(false)
}

#[derive(Debug, Deserialize)]
pub struct AuthPayload {
    pub login: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TeacherOut {
    pub id: i64,
    pub login: String,
}

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AuthPayload>,
) -> Result<(StatusCode, Json<TeacherOut>), AppError> {
    let req_id = request_id_from_headers(&headers);
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("local");
    if !check_rate_limit("auth_register", ip, 20) {
        return Err(AppError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "too many requests",
            req_id,
        ));
    }
    let login = payload.login.trim().to_string();
    if login.len() < 3 || payload.password.len() < 8 {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "invalid login/password",
            req_id,
        ));
    }

    {
        let map = state.db.teachers_by_login.read().await;
        if map.contains_key(&login) {
            return Err(AppError::new(
                StatusCode::CONFLICT,
                "CONFLICT",
                "login already exists",
                req_id,
            ));
        }
    }

    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let hash = Argon2::default()
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "password hash failed", req_id.clone()))?
        .to_string();

    let id = state.db.next_teacher_id();
    let teacher = Teacher { id, login: login.clone(), password_hash: hash };
    state.db.teachers.write().await.insert(id, teacher);
    state.db.teachers_by_login.write().await.insert(login.clone(), id);
    if let Err(err) = state.persist_core_data().await {
        warn!("failed to persist local state after register: {}", err);
    }

    Ok((StatusCode::CREATED, Json(TeacherOut { id, login })))
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<AuthPayload>,
) -> Result<(CookieJar, Json<TeacherOut>), AppError> {
    let req_id = request_id_from_headers(&headers);
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("local");
    if !check_rate_limit("auth_login", ip, 30) {
        return Err(AppError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "too many requests",
            req_id,
        ));
    }
    let login = payload.login.trim().to_string();
    let id = {
        let by_login = state.db.teachers_by_login.read().await;
        by_login.get(&login).copied()
    }
    .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "invalid credentials", req_id.clone()))?;

    let teacher = state
        .db
        .teachers
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "invalid credentials", req_id.clone()))?;

    let parsed_hash = PasswordHash::new(&teacher.password_hash)
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "bad hash", req_id.clone()))?;
    let is_valid = Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_ok();
    if !is_valid {
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "invalid credentials",
            req_id,
        ));
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let csrf_token = uuid::Uuid::new_v4().to_string();
    state.db.sessions.write().await.insert(
        session_id.clone(),
        TeacherSession { teacher_id: id, csrf_token: csrf_token.clone() },
    );

    let cookie = Cookie::build((SESSION_COOKIE, session_id))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .build();
    let csrf_cookie = Cookie::build(("csrf_token", csrf_token))
        .http_only(false)
        .same_site(SameSite::Lax)
        .path("/")
        .build();

    Ok((jar.add(cookie).add(csrf_cookie), Json(TeacherOut { id, login: teacher.login })))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    let req_id = request_id_from_headers(&headers);
    let sid = jar
        .get(SESSION_COOKIE)
        .map(|v| v.value().to_string())
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "no session", req_id.clone()))?;
    state.db.sessions.write().await.remove(&sid);
    Ok((jar.remove(Cookie::from(SESSION_COOKIE)), StatusCode::NO_CONTENT))
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<Json<TeacherOut>, AppError> {
    let req_id = request_id_from_headers(&headers);
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", req_id.clone()))?;
    let teacher = state
        .db
        .teachers
        .read()
        .await
        .get(&teacher_id)
        .cloned()
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", req_id))?;
    Ok(Json(TeacherOut { id: teacher.id, login: teacher.login }))
}

#[derive(Debug, Deserialize)]
pub struct CreateQuizPayload {
    pub title: String,
    pub description: Option<String>,
    pub questions: Vec<crate::models::Question>,
}

#[derive(Debug, Serialize)]
pub struct QuizIdResponse {
    pub quiz_id: i64,
}

pub async fn create_quiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<CreateQuizPayload>,
) -> Result<(StatusCode, Json<QuizIdResponse>), AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;

    let quiz = Quiz {
        title: payload.title,
        description: payload.description,
        questions: payload.questions,
    };
    if let Err(issues) = validate_quiz(&quiz) {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "quiz validation failed",
            request_id_from_headers(&headers),
        )
        .with_details(
            issues
                .into_iter()
                .map(|i| ErrorDetail {
                    field: i.field,
                    issue: i.issue,
                })
                .collect(),
        ));
    }

    let id = state.create_quiz(teacher_id, quiz, None).await;
    Ok((StatusCode::CREATED, Json(QuizIdResponse { quiz_id: id })))
}

#[derive(Debug, Serialize)]
pub struct QuizSummary {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub is_published: bool,
}

#[derive(Debug, Serialize)]
pub struct QuizListResponse {
    pub items: Vec<QuizSummary>,
    pub total: usize,
}

pub async fn list_quizzes(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<Json<QuizListResponse>, AppError> {
    let req_id = request_id_from_headers(&headers);
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", req_id))?;
    let quizzes = state.db.quizzes.read().await;
    let items: Vec<QuizSummary> = quizzes
        .values()
        .filter(|q| q.owner_teacher_id == teacher_id)
        .map(|q| QuizSummary {
            id: q.id,
            title: q.title.clone(),
            description: q.description.clone(),
            is_published: q.is_published,
        })
        .collect();
    Ok(Json(QuizListResponse { total: items.len(), items }))
}

pub async fn get_quiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Result<Json<QuizRecord>, AppError> {
    let req_id = request_id_from_headers(&headers);
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", req_id.clone()))?;
    let quiz = state
        .db
        .quizzes
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "quiz not found", req_id.clone()))?;
    if quiz.owner_teacher_id != teacher_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "access denied", req_id));
    }
    Ok(Json(quiz))
}

pub async fn update_quiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
    Json(payload): Json<CreateQuizPayload>,
) -> Result<Json<QuizIdResponse>, AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;
    let quiz = Quiz {
        title: payload.title,
        description: payload.description,
        questions: payload.questions,
    };
    if let Err(issues) = validate_quiz(&quiz) {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "quiz validation failed",
            request_id_from_headers(&headers),
        )
        .with_details(
            issues
                .into_iter()
                .map(|i| ErrorDetail {
                    field: i.field,
                    issue: i.issue,
                })
                .collect(),
        ));
    }
    let mut quizzes = state.db.quizzes.write().await;
    let item = quizzes
        .get_mut(&id)
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "quiz not found", request_id_from_headers(&headers)))?;
    if item.owner_teacher_id != teacher_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "access denied", request_id_from_headers(&headers)));
    }
    item.title = quiz.title;
    item.description = quiz.description;
    item.questions = quiz.questions;
    drop(quizzes);
    if let Err(err) = state.persist_core_data().await {
        warn!("failed to persist local state after update_quiz: {}", err);
    }
    Ok(Json(QuizIdResponse { quiz_id: id }))
}

pub async fn delete_quiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;
    let mut quizzes = state.db.quizzes.write().await;
    let existing = quizzes
        .get(&id)
        .cloned()
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "quiz not found", request_id_from_headers(&headers)))?;
    if existing.owner_teacher_id != teacher_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "access denied", request_id_from_headers(&headers)));
    }
    quizzes.remove(&id);
    drop(quizzes);
    if let Err(err) = state.persist_core_data().await {
        warn!("failed to persist local state after delete_quiz: {}", err);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn publish_quiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;
    let mut quizzes = state.db.quizzes.write().await;
    let q = quizzes
        .get_mut(&id)
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "quiz not found", request_id_from_headers(&headers)))?;
    if q.owner_teacher_id != teacher_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "access denied", request_id_from_headers(&headers)));
    }
    q.is_published = true;
    drop(quizzes);
    if let Err(err) = state.persist_core_data().await {
        warn!("failed to persist local state after publish_quiz: {}", err);
    }
    Ok(Json(json!({ "published": true })))
}

pub async fn unpublish_quiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;
    let mut quizzes = state.db.quizzes.write().await;
    let q = quizzes
        .get_mut(&id)
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "quiz not found", request_id_from_headers(&headers)))?;
    if q.owner_teacher_id != teacher_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "access denied", request_id_from_headers(&headers)));
    }
    q.is_published = false;
    drop(quizzes);
    if let Err(err) = state.persist_core_data().await {
        warn!("failed to persist local state after unpublish_quiz: {}", err);
    }
    Ok(Json(json!({ "published": false })))
}

pub async fn clone_quiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let req_id = request_id_from_headers(&headers);
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("local");
    if !check_rate_limit("ai_generate", ip, 15) {
        return Err(AppError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "too many requests",
            req_id,
        ));
    }
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;
    let source = state
        .db
        .quizzes
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "quiz not found", request_id_from_headers(&headers)))?;
    if !source.is_published {
        return Err(AppError::new(StatusCode::CONFLICT, "CONFLICT", "quiz is not published", request_id_from_headers(&headers)));
    }
    let quiz_id = state
        .create_quiz(
            teacher_id,
            Quiz {
                title: source.title,
                description: source.description,
                questions: source.questions,
            },
            Some(id),
        )
        .await;
    Ok((StatusCode::CREATED, Json(json!({ "quizId": quiz_id, "sourceQuizId": id }))))
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

pub async fn library_list(
    State(state): State<AppState>,
    query: axum::extract::Query<SearchQuery>,
) -> Json<serde_json::Value> {
    let term = query.q.clone().unwrap_or_default().to_lowercase();
    let quizzes = state.db.quizzes.read().await;
    let items: Vec<_> = quizzes
        .values()
        .filter(|q| q.is_published)
        .filter(|q| {
            term.is_empty()
                || q.title.to_lowercase().contains(&term)
                || q
                    .description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&term))
                    .unwrap_or(false)
        })
        .map(|q| {
            json!({
                "id": q.id,
                "title": q.title,
                "description": q.description,
                "ownerTeacherId": q.owner_teacher_id
            })
        })
        .collect();
    Json(json!({ "items": items, "total": items.len() }))
}

#[derive(Debug, Deserialize)]
pub struct AiGeneratePayload {
    pub topic: String,
    pub grade: Option<String>,
    #[serde(rename = "questionCount")]
    pub question_count: usize,
}

pub async fn ai_generate_quiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<AiGeneratePayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;

    let compiled = jsonschema::draft202012::new(&state.quiz_schema)
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "schema build failed", request_id_from_headers(&headers)))?;
    let mut last_validation_details: Vec<ErrorDetail> = Vec::new();
    let mut last_message = "ai payload does not match schema".to_string();

    for _attempt in 0..2 {
        let raw = state
            .ai_client
            .generate_quiz_json(&payload.topic, payload.grade.as_deref(), payload.question_count)
            .await
            .map_err(|e| {
                AppError::new(
                    StatusCode::BAD_GATEWAY,
                    "UPSTREAM_ERROR",
                    format!("gigachat failed: {}", e),
                    request_id_from_headers(&headers),
                )
            })?;

        let json_value: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                last_message = format!("ai result is not valid json: {}", e);
                last_validation_details.clear();
                continue;
            }
        };

        if compiled.validate(&json_value).is_err() {
            last_validation_details = compiled
                .iter_errors(&json_value)
                .map(|e| ErrorDetail {
                    field: e.instance_path.to_string(),
                    issue: e.to_string(),
                })
                .collect();
            last_message = "ai payload does not match schema".to_string();
            continue;
        }

        let quiz: Quiz = match serde_json::from_value(json_value) {
            Ok(v) => v,
            Err(e) => {
                last_message = format!("cannot decode quiz: {}", e);
                last_validation_details.clear();
                continue;
            }
        };

        if let Err(issues) = validate_quiz(&quiz) {
            last_validation_details = issues
                .into_iter()
                .map(|i| ErrorDetail {
                    field: i.field,
                    issue: i.issue,
                })
                .collect();
            last_message = "quiz validation failed".to_string();
            continue;
        }

        let quiz_id = state.create_quiz(teacher_id, quiz, None).await;
        return Ok((StatusCode::CREATED, Json(json!({ "quizId": quiz_id, "source": "ai" }))));
    }

    Err(AppError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "VALIDATION_ERROR",
        last_message,
        request_id_from_headers(&headers),
    )
    .with_details(last_validation_details))
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionPayload {
    #[serde(rename = "quizId")]
    pub quiz_id: i64,
    #[serde(rename = "gameMode")]
    pub game_mode: String,
}

pub async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<CreateSessionPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;
    if !["platformer", "shooter", "tycoon", "classic"].contains(&payload.game_mode.as_str()) {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "unsupported game mode",
            request_id_from_headers(&headers),
        ));
    }
    let quiz_exists = state.db.quizzes.read().await.contains_key(&payload.quiz_id);
    if !quiz_exists {
        return Err(AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "quiz not found", request_id_from_headers(&headers)));
    }

    let room_code: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect::<String>()
        .to_uppercase();
    let join_token = uuid::Uuid::new_v4().to_string();
    let id = state.db.next_game_session_id();

    let session = SessionRecord {
        id,
        room_code: room_code.clone(),
        join_token: join_token.clone(),
        quiz_id: payload.quiz_id,
        teacher_id,
        status: "waiting".into(),
        game_mode: payload.game_mode,
        participants: HashMap::new(),
        stats: HashMap::new(),
        mistakes: HashMap::new(),
    };
    state.db.game_sessions.write().await.insert(id, session);
    state.db.rooms.write().await.insert(room_code.clone(), id);
    let (tx, _) = broadcast::channel(200);
    state.db.broadcasters.insert(room_code.clone(), tx);

    let join_url = format!("http://localhost:5173/join?room={room_code}");
    Ok((
        StatusCode::CREATED,
        Json(json!({ "sessionId": id, "roomCode": room_code, "joinUrl": join_url, "qrPayload": join_url })),
    ))
}

pub async fn start_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;
    let (room_code, game_mode) = {
        let mut sessions = state.db.game_sessions.write().await;
        let session = sessions
            .get_mut(&id)
            .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "session not found", request_id_from_headers(&headers)))?;
        if session.teacher_id != teacher_id {
            return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "access denied", request_id_from_headers(&headers)));
        }
        session.status = "active".into();
        (session.room_code.clone(), session.game_mode.clone())
    };

    if let Some(sender) = state.db.broadcasters.get(&room_code) {
        let _ = sender.send(WsEnvelope {
            event: "start_quiz".into(),
            payload: json!({ "sessionId": id, "gameMode": game_mode, "startedAt": Utc::now().to_rfc3339() }),
            request_id: None,
            ts: Some(Utc::now().to_rfc3339()),
        });
    }
    Ok(Json(json!({ "status": "active" })))
}

pub async fn end_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let req_id = request_id_from_headers(&headers);
    if !ensure_csrf(&headers, &jar, &state).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "csrf token invalid", req_id));
    }
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", request_id_from_headers(&headers)))?;
    let room_code = {
        let mut sessions = state.db.game_sessions.write().await;
        let session = sessions
            .get_mut(&id)
            .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "session not found", request_id_from_headers(&headers)))?;
        if session.teacher_id != teacher_id {
            return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "access denied", request_id_from_headers(&headers)));
        }
        session.status = "finished".into();
        session.room_code.clone()
    };

    if let Some(sender) = state.db.broadcasters.get(&room_code) {
        let _ = sender.send(WsEnvelope {
            event: "end_quiz".into(),
            payload: json!({ "sessionId": id, "endedAt": Utc::now().to_rfc3339(), "resultsReady": true }),
            request_id: None,
            ts: Some(Utc::now().to_rfc3339()),
        });
    }
    Ok(Json(json!({ "status": "finished" })))
}

pub async fn session_results(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let req_id = request_id_from_headers(&headers);
    let teacher_id = auth_teacher_id(&jar, &state).await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "not logged in", req_id.clone()))?;
    let session = state
        .db
        .game_sessions
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "session not found", req_id.clone()))?;
    if session.teacher_id != teacher_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, "FORBIDDEN", "access denied", req_id));
    }

    let class_correct: u32 = session.stats.values().map(|s| s.correct).sum();
    let class_wrong: u32 = session.stats.values().map(|s| s.wrong).sum();
    let total = class_correct + class_wrong;
    let class_pct = if total == 0 {
        0.0
    } else {
        class_correct as f64 * 100.0 / total as f64
    };

    let students: Vec<_> = session
        .stats
        .values()
        .map(|s| json!({
            "nickname": s.nickname,
            "correct": s.correct,
            "wrong": s.wrong,
            "correctPct": s.correct_pct()
        }))
        .collect();

    let mistakes: Vec<_> = session
        .mistakes
        .iter()
        .map(|(nick, qs)| json!({"nickname": nick, "questions": qs}))
        .collect();

    Ok(Json(json!({
        "session": {"id": session.id, "roomCode": session.room_code, "status": session.status, "gameMode": session.game_mode},
        "classStats": {"correct": class_correct, "wrong": class_wrong, "correctPct": class_pct},
        "studentStats": students,
        "mistakesByStudent": mistakes
    })))
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(room_code): Path<String>,
) -> Response {
    ws.on_upgrade(move |socket| ws_session(socket, state, room_code))
}

async fn ws_session(stream: WebSocket, state: AppState, room_code: String) {
    let session_id = {
        let rooms = state.db.rooms.read().await;
        match rooms.get(&room_code).copied() {
            Some(v) => v,
            None => return,
        }
    };

    let mut receiver = match state.db.broadcasters.get(&room_code) {
        Some(sender) => sender.subscribe(),
        None => return,
    };

    let (mut sender_ws, mut receiver_ws) = stream.split();
    let mut current_nickname: Option<String> = None;

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = receiver.recv().await {
            if let Ok(text) = serde_json::to_string(&msg) {
                if sender_ws.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
        }
    });

    while let Some(Ok(message)) = receiver_ws.next().await {
        if let Message::Text(txt) = message {
            let parsed: Result<WsEnvelope, _> = serde_json::from_str(&txt);
            let Ok(env) = parsed else { continue; };

            if env.event == "join_room" {
                let role = env.payload.get("role").and_then(|v| v.as_str()).unwrap_or("student");
                if role == "student" {
                    let nickname = env
                        .payload
                        .get("nickname")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if nickname.len() >= 2 {
                        current_nickname = Some(nickname.clone());
                        let mut sessions = state.db.game_sessions.write().await;
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.participants.insert(
                                nickname.clone(),
                                ParticipantState {
                                    nickname: nickname.clone(),
                                    join_state: "waiting".into(),
                                    current_question_index: 0,
                                },
                            );
                            session.stats.entry(nickname.clone()).or_insert(StudentStats {
                                nickname: nickname.clone(),
                                correct: 0,
                                wrong: 0,
                            });

                            if let Some(bc) = state.db.broadcasters.get(&room_code) {
                                let participants: Vec<_> = session
                                    .participants
                                    .values()
                                    .map(|p| json!({"nickname": p.nickname, "state": p.join_state}))
                                    .collect();
                                let _ = bc.send(WsEnvelope {
                                    event: "waiting_room_update".into(),
                                    payload: json!({"sessionId": session.id, "participants": participants}),
                                    request_id: env.request_id.clone(),
                                    ts: Some(Utc::now().to_rfc3339()),
                                });
                            }
                        }
                    }
                }
                continue;
            }

            if env.event == "answer_submit" {
                let Some(nickname) = current_nickname.clone() else { continue; };
                let question_id = env
                    .payload
                    .get("questionId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let answer_value = env.payload.get("answer").cloned().unwrap_or(json!({}));
                let submitted: Result<SubmittedAnswer, _> = serde_json::from_value(answer_value);
                let Ok(submitted) = submitted else { continue; };

                let mut sessions = state.db.game_sessions.write().await;
                let Some(session) = sessions.get_mut(&session_id) else { continue; };
                let Some(p) = session.participants.get_mut(&nickname) else { continue; };
                p.join_state = "playing".into();

                let quiz = {
                    let qmap = state.db.quizzes.read().await;
                    qmap.get(&session.quiz_id).cloned()
                };
                let Some(quiz) = quiz else { continue; };
                let maybe_question = quiz.questions.iter().find(|q| q.id == question_id);
                let Some(question) = maybe_question else { continue; };

                let correct = score_answer(question, &submitted);
                if let Some(s) = session.stats.get_mut(&nickname) {
                    if correct {
                        s.correct += 1;
                    } else {
                        s.wrong += 1;
                        session
                            .mistakes
                            .entry(nickname.clone())
                            .or_default()
                            .push(question_id.clone());
                    }
                    // Move forward after any answer (no retry loop).
                    p.current_question_index += 1;
                }

                let class_correct: u32 = session.stats.values().map(|s| s.correct).sum();
                let class_wrong: u32 = session.stats.values().map(|s| s.wrong).sum();
                let total = class_correct + class_wrong;
                let class_pct = if total == 0 {
                    0.0
                } else {
                    class_correct as f64 * 100.0 / total as f64
                };

                if let Some(bc) = state.db.broadcasters.get(&room_code) {
                    let _ = bc.send(WsEnvelope {
                        event: "answer_result".into(),
                        payload: json!({
                            "questionId": question_id,
                            "correct": correct,
                            "nextAction": "continue"
                        }),
                        request_id: env.request_id.clone(),
                        ts: Some(Utc::now().to_rfc3339()),
                    });

                    let students: Vec<_> = session
                        .stats
                        .values()
                        .map(|s| json!({
                            "nickname": s.nickname,
                            "correct": s.correct,
                            "wrong": s.wrong,
                            "correctPct": s.correct_pct()
                        }))
                        .collect();
                    let _ = bc.send(WsEnvelope {
                        event: "stats_update".into(),
                        payload: json!({
                            "class": {"correctPct": class_pct, "wrongPct": 100.0 - class_pct},
                            "students": students
                        }),
                        request_id: env.request_id.clone(),
                        ts: Some(Utc::now().to_rfc3339()),
                    });

                }
            }

            if env.event == "request_question" {
                let Some(nickname) = current_nickname.clone() else { continue; };
                let reason = env
                    .payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("death")
                    .to_string();

                let mut sessions = state.db.game_sessions.write().await;
                let Some(session) = sessions.get_mut(&session_id) else { continue; };
                let Some(participant) = session.participants.get_mut(&nickname) else { continue; };
                let current_idx = participant.current_question_index;
                let quiz = {
                    let qmap = state.db.quizzes.read().await;
                    qmap.get(&session.quiz_id).cloned()
                };
                let Some(quiz) = quiz else { continue; };
                if quiz.questions.is_empty() {
                    continue;
                }
                let question = if let Some(q) = quiz.questions.get(current_idx).cloned() {
                    q
                } else {
                    // In game modes, continue cycling questions instead of ending immediately.
                    if session.game_mode != "classic" {
                        participant.current_question_index = 0;
                        quiz.questions[0].clone()
                    } else {
                        if let Some(bc) = state.db.broadcasters.get(&room_code) {
                            let _ = bc.send(WsEnvelope {
                                event: "end_quiz".into(),
                                payload: json!({ "sessionId": session.id, "endedAt": Utc::now().to_rfc3339(), "resultsReady": true }),
                                request_id: env.request_id.clone(),
                                ts: Some(Utc::now().to_rfc3339()),
                            });
                        }
                        continue;
                    }
                };

                if let Some(bc) = state.db.broadcasters.get(&room_code) {
                    let _ = bc.send(WsEnvelope {
                        event: "question_push".into(),
                        payload: json!({ "question": question, "reason": reason }),
                        request_id: env.request_id.clone(),
                        ts: Some(Utc::now().to_rfc3339()),
                    });
                }
            }
        }
    }

    if let Some(nickname) = current_nickname {
        let mut sessions = state.db.game_sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            if let Some(p) = session.participants.get_mut(&nickname) {
                p.join_state = "left".into();
            }
        }
    }

    send_task.abort();
    info!("ws disconnected for room {}", room_code);
}

use futures::{SinkExt, StreamExt};
