pub mod error;
pub mod handlers;
pub mod models;
pub mod routes;
pub mod state;
pub mod ws_protocol;

use std::sync::Arc;

pub fn build_state() -> anyhow::Result<state::AppState> {
    let schema_raw = include_str!("../contracts/ai_quiz.schema.json");
    let schema: serde_json::Value = serde_json::from_str(schema_raw)?;
    let ai_client: Arc<dyn state::AiQuizClient> = if let Some(real) = state::GigaChatAiClient::from_env() {
        Arc::new(real)
    } else {
        Arc::new(state::MockAiClient)
    };
    Ok(state::AppState::new(ai_client, schema))
}
