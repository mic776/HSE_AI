use crate::models::{Quiz, StudentStats};
use crate::ws_protocol::WsEnvelope;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use dashmap::DashMap;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::{fs, path::Path};
use tokio::process::Command;
use tokio::sync::{broadcast, RwLock};
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Teacher {
    pub id: i64,
    pub login: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizRecord {
    pub id: i64,
    pub owner_teacher_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub questions: Vec<crate::models::Question>,
    pub is_published: bool,
    pub source_quiz_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantState {
    pub nickname: String,
    pub join_state: String,
    pub current_question_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: i64,
    pub room_code: String,
    pub join_token: String,
    pub quiz_id: i64,
    pub teacher_id: i64,
    pub status: String,
    pub game_mode: String,
    pub participants: HashMap<String, ParticipantState>,
    pub stats: HashMap<String, StudentStats>,
    pub mistakes: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct TeacherSession {
    pub teacher_id: i64,
    pub csrf_token: String,
}

pub struct InMemoryDb {
    pub teachers: RwLock<HashMap<i64, Teacher>>,
    pub teachers_by_login: RwLock<HashMap<String, i64>>,
    pub sessions: RwLock<HashMap<String, TeacherSession>>,
    pub quizzes: RwLock<HashMap<i64, QuizRecord>>,
    pub game_sessions: RwLock<HashMap<i64, SessionRecord>>,
    pub rooms: RwLock<HashMap<String, i64>>,
    pub broadcasters: DashMap<String, broadcast::Sender<WsEnvelope>>,
    next_teacher_id: AtomicI64,
    next_quiz_id: AtomicI64,
    next_session_id: AtomicI64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistentSnapshot {
    teachers: HashMap<i64, Teacher>,
    teachers_by_login: HashMap<String, i64>,
    quizzes: HashMap<i64, QuizRecord>,
    next_teacher_id: i64,
    next_quiz_id: i64,
    next_session_id: i64,
}

impl InMemoryDb {
    pub fn new(snapshot_path: Option<&str>) -> Self {
        let snapshot = snapshot_path.and_then(|path| {
            let raw = fs::read_to_string(path).ok()?;
            match serde_json::from_str::<PersistentSnapshot>(&raw) {
                Ok(s) => Some(s),
                Err(err) => {
                    warn!("failed to read local snapshot {}: {}", path, err);
                    None
                }
            }
        });

        let teachers = snapshot
            .as_ref()
            .map(|s| s.teachers.clone())
            .unwrap_or_default();
        let teachers_by_login = snapshot
            .as_ref()
            .map(|s| s.teachers_by_login.clone())
            .unwrap_or_default();
        let quizzes = snapshot
            .as_ref()
            .map(|s| s.quizzes.clone())
            .unwrap_or_default();
        let next_teacher_id = snapshot.as_ref().map(|s| s.next_teacher_id).unwrap_or(1).max(
            teachers.keys().max().copied().unwrap_or(0) + 1,
        );
        let next_quiz_id = snapshot.as_ref().map(|s| s.next_quiz_id).unwrap_or(1).max(
            quizzes.keys().max().copied().unwrap_or(0) + 1,
        );
        let next_session_id = snapshot.as_ref().map(|s| s.next_session_id).unwrap_or(1).max(1);

        Self {
            teachers: RwLock::new(teachers),
            teachers_by_login: RwLock::new(teachers_by_login),
            sessions: RwLock::new(HashMap::new()),
            quizzes: RwLock::new(quizzes),
            game_sessions: RwLock::new(HashMap::new()),
            rooms: RwLock::new(HashMap::new()),
            broadcasters: DashMap::new(),
            next_teacher_id: AtomicI64::new(next_teacher_id),
            next_quiz_id: AtomicI64::new(next_quiz_id),
            next_session_id: AtomicI64::new(next_session_id),
        }
    }

    pub fn next_teacher_id(&self) -> i64 {
        self.next_teacher_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn next_quiz_id(&self) -> i64 {
        self.next_quiz_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn next_game_session_id(&self) -> i64 {
        self.next_session_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn snapshot(&self) -> PersistentSnapshot {
        PersistentSnapshot {
            teachers: self.teachers.read().await.clone(),
            teachers_by_login: self.teachers_by_login.read().await.clone(),
            quizzes: self.quizzes.read().await.clone(),
            next_teacher_id: self.next_teacher_id.load(Ordering::SeqCst),
            next_quiz_id: self.next_quiz_id.load(Ordering::SeqCst),
            next_session_id: self.next_session_id.load(Ordering::SeqCst),
        }
    }
}

pub trait AiQuizClient: Send + Sync {
    fn generate_quiz_json(
        &self,
        topic: &str,
        grade: Option<&str>,
        question_count: usize,
    ) -> BoxFuture<'static, anyhow::Result<String>>;
}

#[derive(Clone)]
pub struct MockAiClient;

impl AiQuizClient for MockAiClient {
    fn generate_quiz_json(
        &self,
        topic: &str,
        _grade: Option<&str>,
        question_count: usize,
    ) -> BoxFuture<'static, anyhow::Result<String>> {
        let topic = topic.to_string();
        Box::pin(async move {
            let mut questions = Vec::new();
            for idx in 0..question_count.max(1) {
                questions.push(serde_json::json!({
                    "id": format!("q{}", idx + 1),
                    "type": "single",
                    "prompt": format!("{}: вопрос {}", topic, idx + 1),
                    "options": [
                        {"id": "o1", "text": "Верно"},
                        {"id": "o2", "text": "Неверно"}
                    ],
                    "answer": {"optionId": "o1"}
                }));
            }
            let payload = serde_json::json!({
                "title": format!("Квиз: {}", topic),
                "description": "Сгенерировано ИИ",
                "questions": questions
            });
            Ok(payload.to_string())
        })
    }
}

#[derive(Clone)]
pub struct GigaChatAiClient {
    pub python_bin: String,
    pub script_path: String,
    pub base_url: String,
    pub bearer: Option<String>,
    pub credentials: Option<String>,
    pub auth_url: String,
    pub scope: String,
    pub model: String,
    pub timeout_secs: u64,
    pub system_prompt_path: String,
}

impl GigaChatAiClient {
    pub fn from_env() -> Option<Self> {
        let bearer = std::env::var("BEARER")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                std::env::var("GIGACHAT_BEARER")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            });
        let credentials = std::env::var("GIGACHAT_CREDENTIALS")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                let cid = std::env::var("GIGACHAT_CLIENT_ID").ok()?;
                let sec = std::env::var("GIGACHAT_CLIENT_SECRET").ok()?;
                Some(STANDARD.encode(format!("{}:{}", cid, sec)))
            })
            .or_else(|| bearer.clone());
        if bearer.is_none() && credentials.is_none() {
            return None;
        }
        let mut base_url = std::env::var("GIGACHAT_BASE_URL")
            .unwrap_or_else(|_| "https://gigachat.devices.sberbank.ru".to_string());
        if !base_url.contains("/api/v1") {
            base_url = format!("{}/api/v1", base_url.trim_end_matches('/'));
        }
        let auth_url = std::env::var("GIGACHAT_AUTH_URL")
            .unwrap_or_else(|_| "https://ngw.devices.sberbank.ru:9443/api/v2/oauth".to_string());
        let scope = std::env::var("GIGACHAT_SCOPE").unwrap_or_else(|_| "GIGACHAT_API_PERS".to_string());
        let model = std::env::var("GIGACHAT_MODEL").unwrap_or_else(|_| "GigaChat".to_string());
        let timeout_secs = std::env::var("GIGACHAT_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);
        let python_bin = std::env::var("PYTHON_BIN").unwrap_or_else(|_| "python3".to_string());
        let script_path = format!("{}/scripts/gigachat_generate.py", env!("CARGO_MANIFEST_DIR"));
        let system_prompt_path = format!("{}/../docs/gigachat_system_prompt.txt", env!("CARGO_MANIFEST_DIR"));

        Some(Self {
            python_bin,
            script_path,
            base_url,
            bearer,
            credentials,
            auth_url,
            scope,
            model,
            timeout_secs,
            system_prompt_path,
        })
    }
}

impl AiQuizClient for GigaChatAiClient {
    fn generate_quiz_json(
        &self,
        topic: &str,
        grade: Option<&str>,
        question_count: usize,
    ) -> BoxFuture<'static, anyhow::Result<String>> {
        let python_bin = self.python_bin.clone();
        let script_path = self.script_path.clone();
        let base_url = self.base_url.clone();
        let bearer = self.bearer.clone();
        let credentials = self.credentials.clone();
        let auth_url = self.auth_url.clone();
        let scope = self.scope.clone();
        let model = self.model.clone();
        let system_prompt_path = self.system_prompt_path.clone();
        let timeout_secs = self.timeout_secs;
        let grade_text = grade.unwrap_or("не указан").to_string();
        let topic_text = topic.to_string();
        let count = question_count.max(1);

        Box::pin(async move {
            let mut cmd = Command::new(&python_bin);
            cmd.arg(&script_path)
                .arg("--topic")
                .arg(&topic_text)
                .arg("--grade")
                .arg(&grade_text)
                .arg("--count")
                .arg(count.to_string())
                .arg("--model")
                .arg(&model)
                .arg("--base-url")
                .arg(&base_url)
                .arg("--auth-url")
                .arg(&auth_url)
                .arg("--scope")
                .arg(&scope)
                .arg("--timeout")
                .arg(timeout_secs.to_string())
                .arg("--system-prompt-file")
                .arg(&system_prompt_path);

            if let Some(credentials) = credentials {
                cmd.arg("--credentials").arg(credentials);
            }
            if let Some(bearer) = bearer {
                cmd.env("BEARER", bearer);
            }

            let output = cmd.output().await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                anyhow::bail!("gigachat python client failed: {}", stderr);
            }

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let trimmed = stdout.trim();
            let cleaned = if trimmed.starts_with("```") {
                trimmed
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim()
                    .to_string()
            } else {
                trimmed.to_string()
            };
            if cleaned.is_empty() {
                anyhow::bail!("gigachat returned empty content");
            }
            Ok(cleaned)
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<InMemoryDb>,
    pub ai_client: Arc<dyn AiQuizClient>,
    pub quiz_schema: Arc<serde_json::Value>,
    pub local_state_path: Option<String>,
}

impl AppState {
    pub fn new(ai_client: Arc<dyn AiQuizClient>, quiz_schema: serde_json::Value) -> Self {
        let local_state_path = std::env::var("LOCAL_STATE_PATH")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| Some(format!("{}/local_state.json", env!("CARGO_MANIFEST_DIR"))));
        Self {
            db: Arc::new(InMemoryDb::new(local_state_path.as_deref())),
            ai_client,
            quiz_schema: Arc::new(quiz_schema),
            local_state_path,
        }
    }

    pub async fn create_quiz(&self, teacher_id: i64, quiz: Quiz, source_quiz_id: Option<i64>) -> i64 {
        let id = self.db.next_quiz_id();
        let record = QuizRecord {
            id,
            owner_teacher_id: teacher_id,
            title: quiz.title,
            description: quiz.description,
            questions: quiz.questions,
            is_published: false,
            source_quiz_id,
        };
        self.db.quizzes.write().await.insert(id, record);
        if let Err(err) = self.persist_core_data().await {
            warn!("failed to persist local state after create_quiz: {}", err);
        }
        id
    }

    pub async fn persist_core_data(&self) -> anyhow::Result<()> {
        let Some(path) = self.local_state_path.as_ref() else {
            return Ok(());
        };
        let snapshot = self.db.snapshot().await;
        let serialized = serde_json::to_vec_pretty(&snapshot)?;
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, serialized).await?;
        Ok(())
    }
}
