use futures::{SinkExt, StreamExt};
use quiz_backend::{build_state, routes::build_router};
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

async fn spawn_server() -> (String, reqwest::Client) {
    std::env::remove_var("BEARER");
    std::env::remove_var("GIGACHAT_BEARER");
    let state = build_state().expect("state");
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    (format!("http://{}", addr), client)
}

async fn auth(base: &str, client: &reqwest::Client, login: &str) -> String {
    client
        .post(format!("{}/api/v1/auth/register", base))
        .json(&json!({"login": login, "password": "password123"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!("{}/api/v1/auth/login", base))
        .json(&json!({"login": login, "password": "password123"}))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let csrf = resp
        .cookies()
        .find(|c| c.name() == "csrf_token")
        .map(|c| c.value().to_string())
        .unwrap();
    csrf
}

fn csrf_headers(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("x-csrf-token", HeaderValue::from_str(token).unwrap());
    h
}

fn sample_quiz_payload() -> serde_json::Value {
    json!({
        "title": "Математика",
        "description": "Базовый тест",
        "questions": [
            {
                "id": "q1",
                "type": "open",
                "prompt": "2+2",
                "answer": {"text": "4"}
            },
            {
                "id": "q2",
                "type": "single",
                "prompt": "Столица Франции",
                "options": [
                    {"id": "o1", "text": "Париж"},
                    {"id": "o2", "text": "Берлин"}
                ],
                "answer": {"optionId": "o1"}
            },
            {
                "id": "q3",
                "type": "multi",
                "prompt": "Выбери четные",
                "options": [
                    {"id": "o1", "text": "2"},
                    {"id": "o2", "text": "3"},
                    {"id": "o3", "text": "4"}
                ],
                "answer": {"optionIds": ["o1", "o3"]}
            }
        ]
    })
}

#[tokio::test]
async fn register_login_quiz_publish_search_clone_flow() {
    let (base, client1) = spawn_server().await;
    let csrf1 = auth(&base, &client1, "teacher1").await;

    let create = client1
        .post(format!("{}/api/v1/quizzes", base))
        .headers(csrf_headers(&csrf1))
        .json(&sample_quiz_payload())
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let quiz_id = create.json::<serde_json::Value>().await.unwrap()["quiz_id"].as_i64().unwrap();

    let publish = client1
        .post(format!("{}/api/v1/quizzes/{}/publish", base, quiz_id))
        .headers(csrf_headers(&csrf1))
        .send()
        .await
        .unwrap();
    assert_eq!(publish.status(), 200);

    let library = client1
        .get(format!("{}/api/v1/library/quizzes?q=мат", base))
        .send()
        .await
        .unwrap();
    assert_eq!(library.status(), 200);
    assert!(library.text().await.unwrap().contains("Математика"));

    let client2 = reqwest::Client::builder().cookie_store(true).build().unwrap();
    let csrf2 = auth(&base, &client2, "teacher2").await;
    let clone = client2
        .post(format!("{}/api/v1/quizzes/{}/clone", base, quiz_id))
        .headers(csrf_headers(&csrf2))
        .send()
        .await
        .unwrap();
    assert_eq!(clone.status(), 201);
}

#[tokio::test]
async fn ai_generate_and_save() {
    let (base, client) = spawn_server().await;
    let csrf = auth(&base, &client, "ai_teacher").await;

    let resp = client
        .post(format!("{}/api/v1/ai/generate-quiz", base))
        .headers(csrf_headers(&csrf))
        .json(&json!({"topic": "История", "grade": "8", "questionCount": 3}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body = resp.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["source"], "ai");
}

#[tokio::test]
async fn session_ws_start_submit_stats_end_results() {
    let (base, client) = spawn_server().await;
    let csrf = auth(&base, &client, "live_teacher").await;

    let create_quiz = client
        .post(format!("{}/api/v1/quizzes", base))
        .headers(csrf_headers(&csrf))
        .json(&sample_quiz_payload())
        .send()
        .await
        .unwrap();
    let quiz_id = create_quiz.json::<serde_json::Value>().await.unwrap()["quiz_id"].as_i64().unwrap();

    let session = client
        .post(format!("{}/api/v1/sessions", base))
        .headers(csrf_headers(&csrf))
        .json(&json!({"quizId": quiz_id, "gameMode": "platformer"}))
        .send()
        .await
        .unwrap();
    assert_eq!(session.status(), 201);
    let session_json = session.json::<serde_json::Value>().await.unwrap();
    let session_id = session_json["sessionId"].as_i64().unwrap();
    let room = session_json["roomCode"].as_str().unwrap().to_string();

    let ws_url = base.replace("http://", "ws://");
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("{}/ws/sessions/{}", ws_url, room))
        .await
        .unwrap();

    ws.send(Message::Text(
        json!({"event":"join_room","payload":{"role":"student","nickname":"Ира"}}).to_string(),
    ))
    .await
    .unwrap();

    let _waiting = ws.next().await.unwrap().unwrap();

    let started = client
        .post(format!("{}/api/v1/sessions/{}/start", base, session_id))
        .headers(csrf_headers(&csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(started.status(), 200);

    let _start_msg = ws.next().await.unwrap().unwrap();

    ws.send(Message::Text(
        json!({
            "event":"answer_submit",
            "payload": {"questionId": "q1", "answer": {"text": "5"}}
        })
        .to_string(),
    ))
    .await
    .unwrap();

    let msg1 = ws.next().await.unwrap().unwrap();
    let msg2 = ws.next().await.unwrap().unwrap();
    let txt1 = msg1.into_text().unwrap();
    let txt2 = msg2.into_text().unwrap();
    assert!(txt1.contains("answer_result") || txt2.contains("answer_result"));
    assert!(txt1.contains("stats_update") || txt2.contains("stats_update"));

    let ended = client
        .post(format!("{}/api/v1/sessions/{}/end", base, session_id))
        .headers(csrf_headers(&csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(ended.status(), 200);

    let results = client
        .get(format!("{}/api/v1/sessions/{}/results", base, session_id))
        .send()
        .await
        .unwrap();
    assert_eq!(results.status(), 200);
    let r = results.text().await.unwrap();
    assert!(r.contains("mistakesByStudent"));
}
