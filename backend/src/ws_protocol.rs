use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEnvelope {
    pub event: String,
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_serialization_roundtrip() {
        let env = WsEnvelope {
            event: "waiting_room_update".into(),
            payload: serde_json::json!({"participants": [{"nickname": "A"}] }),
            request_id: Some("abc".into()),
            ts: Some("2026-01-01T00:00:00Z".into()),
        };
        let raw = serde_json::to_string(&env).unwrap();
        let parsed: WsEnvelope = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.event, "waiting_room_update");
        assert_eq!(parsed.request_id.unwrap(), "abc");
    }
}
