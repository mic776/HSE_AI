# HoroQuiz (local network ready)

Production-grade минимально-рабочая версия платформы квизов:
- учитель: регистрация/логин, CRUD квизов, публикация, библиотека, clone, запуск live-сессии
- ученик: вход без аккаунта по коду комнаты/QR + ник, waiting room, игра + вопросы
- realtime: WebSocket (waiting room, start, question push, answer submit/result, stats update, end)
- AI: генерация квиза через отдельный модуль клиента (в проекте mock-клиент для тестов)

## Репозиторий

- `backend` — Rust + axum + sqlx + ws + auth/rate-limit/error-model/tests
- `frontend` — React + Vite + TypeScript + Tailwind + Framer Motion + Canvas mini-games
- `docs/architecture.md` — схема БД, API/WS контракты, слои
- `backend/contracts/ai_quiz.schema.json` — строгая JSON schema для AI
- `docs/gigachat_system_prompt.txt` — system prompt

## Env

Используйте `.env.example` как шаблон:

```env
DATABASE_URL=mysql://user:password@localhost:3306/quiz_app
SESSION_SECRET=change_me_to_long_random_secret
RUST_LOG=info
CORS_ORIGIN=http://localhost:5173
COOKIE_SECURE=false
LOCAL_STATE_PATH=backend/local_state.json

GIGACHAT_BASE_URL=https://gigachat.devices.sberbank.ru
GIGACHAT_AUTH_URL=https://ngw.devices.sberbank.ru:9443/api/v2/oauth
GIGACHAT_CLIENT_ID=your_client_id
GIGACHAT_CLIENT_SECRET=your_client_secret
GIGACHAT_SCOPE=GIGACHAT_API_PERS
GIGACHAT_CREDENTIALS=base64(client_id:client_secret)
BEARER=optional_access_token
GIGACHAT_MODEL=GigaChat
GIGACHAT_TIMEOUT_SECS=30
```

## Миграции MySQL

1. Создайте БД и пользователя в MySQL.
2. Установите `sqlx-cli` (один раз):

```bash
cargo install sqlx-cli --no-default-features --features mysql
```

3. Примените миграции:

```bash
cd backend
sqlx migrate run
```

`backend/migrations/20260226180000_init.sql` содержит полную схему таблиц.

## Backend (Rust)

```bash
cd backend
cargo test
cargo run
```

По умолчанию слушает: `0.0.0.0:8080`.

Для AI-генерации через официальный Python SDK:

```bash
python3 -m pip install gigachat
```

## Frontend (React)

```bash
cd frontend
npm i
npm run dev -- --host
npm run build
```

Frontend доступен на `http://localhost:5173` и в локальной сети (через `--host`).

## Локальная сеть (проверка)

1. Запустите backend на машине хоста: `cargo run`.
2. Запустите frontend на машине хоста: `npm run dev -- --host`.
3. Откройте `http://<IP-хоста>:5173` на другом устройстве в той же сети.
4. Учитель создаёт комнату, ученик подключается по коду/QR.

## Тесты

Backend:
- Unit:
  - валидация quiz/question/answer
  - scoring open/single/multi
  - student stats процент
  - WS envelope serialization
- Integration:
  - register/login
  - create quiz
  - publish + library search + clone
  - ai generate + validate + save
  - create session + join student + start + submit + stats + end + results

Запуск:

```bash
cd backend
cargo test
```

## GigaChat integration

- Основные данные (`аккаунты/квизы/публикации`) сохраняются в локальный snapshot-файл `LOCAL_STATE_PATH` и переживают перезапуск backend.
- Миграции MySQL и sqlx-инициализация также присутствуют.
- Backend вызывает официальный Python SDK `gigachat` (скрипт `backend/scripts/gigachat_generate.py`) и использует `Chat` + `messages` + `stream=false`.
- В `main` загружается `.env` через `dotenvy`.
- Авторизация в SDK:
  - сначала пробует `BEARER`/`GIGACHAT_BEARER` как access token,
  - если не вышло, пробует `GIGACHAT_CREDENTIALS` или пару `GIGACHAT_CLIENT_ID` + `GIGACHAT_CLIENT_SECRET` через `auth_url/scope`.
- Если не заданы ни access token, ни credentials, автоматически используется mock-клиент (это сохраняет оффлайн-тесты стабильными).
