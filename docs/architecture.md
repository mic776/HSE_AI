# Архитектура и контракты

## 1) Схема БД MySQL

### Таблицы

1. `teachers`
- `id` BIGINT PK AI
- `login` VARCHAR(64) NOT NULL UNIQUE
- `password_hash` VARCHAR(255) NOT NULL
- `created_at` DATETIME(3) NOT NULL
- `updated_at` DATETIME(3) NOT NULL

2. `teacher_sessions`
- `id` CHAR(36) PK (UUID)
- `teacher_id` BIGINT NOT NULL FK -> `teachers.id`
- `csrf_token` CHAR(36) NOT NULL
- `expires_at` DATETIME(3) NOT NULL
- `created_at` DATETIME(3) NOT NULL
- index: (`teacher_id`), (`expires_at`)

3. `quizzes`
- `id` BIGINT PK AI
- `owner_teacher_id` BIGINT NOT NULL FK -> `teachers.id`
- `title` VARCHAR(255) NOT NULL
- `description` TEXT NULL
- `is_published` BOOLEAN NOT NULL DEFAULT FALSE
- `source_quiz_id` BIGINT NULL FK -> `quizzes.id` (для clone)
- `created_at` DATETIME(3) NOT NULL
- `updated_at` DATETIME(3) NOT NULL
- index: (`owner_teacher_id`), (`is_published`), (`source_quiz_id`)

4. `quiz_questions`
- `id` BIGINT PK AI
- `quiz_id` BIGINT NOT NULL FK -> `quizzes.id`
- `external_id` VARCHAR(64) NOT NULL
- `q_type` ENUM('open','single','multi') NOT NULL
- `prompt` TEXT NOT NULL
- `position` INT NOT NULL
- UNIQUE (`quiz_id`, `external_id`)
- index: (`quiz_id`, `position`)

5. `quiz_options`
- `id` BIGINT PK AI
- `question_id` BIGINT NOT NULL FK -> `quiz_questions.id`
- `external_id` VARCHAR(64) NOT NULL
- `text` TEXT NOT NULL
- `position` INT NOT NULL
- UNIQUE (`question_id`, `external_id`)
- index: (`question_id`, `position`)

6. `quiz_answers`
- `id` BIGINT PK AI
- `question_id` BIGINT NOT NULL FK -> `quiz_questions.id` UNIQUE
- `open_text` TEXT NULL
- `single_option_external_id` VARCHAR(64) NULL
- `multi_option_external_ids` JSON NULL
- CHECK: ровно одно из полей ответа заполнено в зависимости от `q_type`

7. `quiz_publications`
- `id` BIGINT PK AI
- `quiz_id` BIGINT NOT NULL UNIQUE FK -> `quizzes.id`
- `published_by_teacher_id` BIGINT NOT NULL FK -> `teachers.id`
- `published_at` DATETIME(3) NOT NULL
- FULLTEXT index: quiz title/description через join таблицу `quizzes`

8. `game_sessions`
- `id` BIGINT PK AI
- `room_code` VARCHAR(12) NOT NULL UNIQUE
- `join_token` CHAR(36) NOT NULL UNIQUE
- `quiz_id` BIGINT NOT NULL FK -> `quizzes.id`
- `teacher_id` BIGINT NOT NULL FK -> `teachers.id`
- `status` ENUM('waiting','active','finished') NOT NULL
- `game_mode` ENUM('platformer','shooter','tycoon') NOT NULL
- `started_at` DATETIME(3) NULL
- `ended_at` DATETIME(3) NULL
- `created_at` DATETIME(3) NOT NULL
- index: (`teacher_id`, `status`), (`quiz_id`)

9. `session_participants`
- `id` BIGINT PK AI
- `session_id` BIGINT NOT NULL FK -> `game_sessions.id`
- `nickname` VARCHAR(64) NOT NULL
- `join_state` ENUM('waiting','playing','left') NOT NULL
- `connected_at` DATETIME(3) NOT NULL
- `left_at` DATETIME(3) NULL
- UNIQUE (`session_id`, `nickname`)
- index: (`session_id`, `join_state`)

10. `session_question_states`
- `id` BIGINT PK AI
- `session_id` BIGINT NOT NULL FK -> `game_sessions.id`
- `participant_id` BIGINT NOT NULL FK -> `session_participants.id`
- `question_id` BIGINT NOT NULL FK -> `quiz_questions.id`
- `attempts` INT NOT NULL DEFAULT 0
- `is_correct` BOOLEAN NOT NULL DEFAULT FALSE
- `first_attempt_at` DATETIME(3) NULL
- `last_attempt_at` DATETIME(3) NULL
- UNIQUE (`session_id`, `participant_id`, `question_id`)

11. `session_answers`
- `id` BIGINT PK AI
- `session_id` BIGINT NOT NULL FK -> `game_sessions.id`
- `participant_id` BIGINT NOT NULL FK -> `session_participants.id`
- `question_id` BIGINT NOT NULL FK -> `quiz_questions.id`
- `attempt_no` INT NOT NULL
- `answer_payload` JSON NOT NULL
- `is_correct` BOOLEAN NOT NULL
- `answered_at` DATETIME(3) NOT NULL
- UNIQUE (`session_id`, `participant_id`, `question_id`, `attempt_no`)
- index: (`session_id`, `participant_id`), (`session_id`, `question_id`)

12. `session_stats_aggregate`
- `id` BIGINT PK AI
- `session_id` BIGINT NOT NULL FK -> `game_sessions.id`
- `participant_id` BIGINT NULL FK -> `session_participants.id` (NULL = класс)
- `correct_count` INT NOT NULL DEFAULT 0
- `wrong_count` INT NOT NULL DEFAULT 0
- `correct_pct` DECIMAL(5,2) NOT NULL DEFAULT 0
- `updated_at` DATETIME(3) NOT NULL
- UNIQUE (`session_id`, `participant_id`)

## 2) REST API контракты

База: `/api/v1`, JSON везде.

### Auth

1. `POST /auth/register`
- req: `{ "login": "string", "password": "string" }`
- res 201: `{ "id": number, "login": "string" }`
- errors: `409` login exists, `400` validation

2. `POST /auth/login`
- req: `{ "login": "string", "password": "string" }`
- res 200: `{ "id": number, "login": "string" }` + cookie session + csrf token
- errors: `401` invalid creds, `429` rate limit

3. `POST /auth/logout`
- req: csrf header required
- res 204

4. `GET /auth/me`
- res 200: `{ "id": number, "login": "string" }`
- error: `401`

### Quizzes (teacher-owned)

1. `POST /quizzes`
- req: `{ "title": "string", "description": "string?", "questions": Question[] }`
- res 201: `{ "quizId": number }`

2. `GET /quizzes`
- query: `page`, `limit`, `search?`
- res 200: `{ "items": QuizSummary[], "total": number }`

3. `GET /quizzes/{id}`
- res 200: `QuizDetail`
- errors: `404`, `403`

4. `PUT /quizzes/{id}`
- req: полная замена quiz payload
- res 200: `{ "quizId": number }`

5. `DELETE /quizzes/{id}`
- res 204

6. `POST /quizzes/{id}/publish`
- res 200: `{ "published": true }`

7. `POST /quizzes/{id}/unpublish`
- res 200: `{ "published": false }`

8. `POST /quizzes/{id}/clone`
- доступно для published quizzes другого учителя
- res 201: `{ "quizId": number, "sourceQuizId": number }`

### Library

1. `GET /library/quizzes`
- query: `q`, `page`, `limit`
- res 200: `{ "items": PublishedQuizSummary[], "total": number }`

### AI

1. `POST /ai/generate-quiz`
- req: `{ "topic": "string", "grade": "string?", "questionCount": number }`
- flow: GigaChat -> strict JSON string -> backend validation -> save draft quiz
- res 201: `{ "quizId": number, "source": "ai" }`
- errors: `422` invalid model JSON, `429` rate limit, `502` provider failure

### Sessions

1. `POST /sessions`
- req: `{ "quizId": number, "gameMode": "platformer|shooter|tycoon" }`
- res 201: `{ "sessionId": number, "roomCode": "string", "joinUrl": "string", "qrPayload": "string" }`

2. `POST /sessions/{id}/start`
- res 200: `{ "status": "active" }`

3. `POST /sessions/{id}/end`
- res 200: `{ "status": "finished" }`

4. `GET /sessions/{id}/results`
- res 200: `{ "session": ..., "classStats": ..., "studentStats": [...], "mistakesByStudent": [...] }`

## 3) WebSocket контракты

URL: `/ws/sessions/{roomCode}`

Envelope:
- request: `{ "event": "event_name", "payload": {...}, "requestId": "uuid?" }`
- response: `{ "event": "event_name", "payload": {...}, "requestId": "uuid?", "ts": "ISO-8601" }`

### Client -> Server

1. `join_room`
- payload (student): `{ "role": "student", "nickname": "string" }`
- payload (teacher): `{ "role": "teacher", "csrf": "string" }`

2. `answer_submit`
- payload: `{ "questionId": "string", "answer": {...} }`

3. `request_stats`
- payload: `{}` (teacher only)

### Server -> Client

1. `waiting_room_update`
- payload: `{ "sessionId": number, "participants": [{ "nickname": "string", "state": "waiting|playing|left" }] }`

2. `start_quiz`
- payload: `{ "sessionId": number, "startedAt": "ISO-8601" }`

3. `question_push`
- payload: `{ "question": QuestionPublic, "reason": "death|level_up|retry" }`

4. `answer_result`
- payload: `{ "questionId": "string", "correct": boolean, "nextAction": "retry|continue" }`

5. `stats_update`
- payload: `{ "class": {"correctPct": number, "wrongPct": number}, "students": [...] }`

6. `end_quiz`
- payload: `{ "sessionId": number, "endedAt": "ISO-8601", "resultsReady": true }`

## 4) Валидация и единая error model

### Общие правила

- Все строковые поля: `trim`, не пустые.
- `login`: 3..64, `[a-zA-Z0-9_.-]`.
- `password`: минимум 8.
- `nickname`: 2..64, без control chars.
- Quiz JSON строго по schema (`backend/contracts/ai_quiz.schema.json`).
- Для `multi`: минимум 1 правильный option id.

### Формат ошибок

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Human-readable summary",
    "details": [
      { "field": "questions[0].prompt", "issue": "must not be empty" }
    ],
    "requestId": "uuid"
  }
}
```

Коды/HTTP:
- `VALIDATION_ERROR` -> 400
- `UNAUTHORIZED` -> 401
- `FORBIDDEN` -> 403
- `NOT_FOUND` -> 404
- `CONFLICT` -> 409
- `RATE_LIMITED` -> 429
- `UPSTREAM_ERROR` -> 502
- `INTERNAL_ERROR` -> 500

## 5) Слои backend

1. `routes` — маршрутизация и middleware (auth, csrf, rate-limit, request-id, logging).
2. `handlers` — HTTP/WS адаптеры, DTO mapping.
3. `services` — бизнес-логика: auth, quiz lifecycle, live session orchestration, scoring, AI import.
4. `repositories` — sqlx-запросы и транзакции.
5. `db` — pool, migrations runner, typed query utilities.

## 6) Frontend: роуты/страницы/состояния

### Teacher routes

- `/login`, `/register`
- `/teacher/dashboard`
- `/teacher/quizzes/new`
- `/teacher/quizzes/:id/edit`
- `/teacher/library`
- `/teacher/sessions/:id/waiting`
- `/teacher/sessions/:id/live`
- `/teacher/sessions/:id/results`

### Student routes

- `/join` (ввод room code / QR redirect)
- `/wait/:roomCode`
- `/play/:roomCode`
- `/answer/:roomCode`
- `/done/:roomCode`

### Frontend state slices

- `authState`
- `quizBuilderState`
- `libraryState`
- `liveSessionState` (WS status, participants, current question, stats)
- `gameState` (platformer/shooter/tycoon + event triggers)

## 7) JSON схема ИИ-квиза

Схема вынесена в `backend/contracts/ai_quiz.schema.json` и является единственным источником валидации AI-ответа.

## 8) System prompt GigaChat

Промпт вынесен в `docs/gigachat_system_prompt.txt`.
