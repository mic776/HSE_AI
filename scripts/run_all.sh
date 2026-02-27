#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/backend"
FRONTEND_DIR="$ROOT_DIR/frontend"

RUN_TESTS=false
RUN_MIGRATIONS=false

for arg in "$@"; do
  case "$arg" in
    --test) RUN_TESTS=true ;;
    --migrate) RUN_MIGRATIONS=true ;;
    *)
      echo "Unknown flag: $arg"
      echo "Usage: $0 [--test] [--migrate]"
      exit 1
      ;;
  esac
done

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1"
    exit 1
  fi
}

need_cmd cargo
need_cmd npm
need_cmd curl

if [[ "$RUN_MIGRATIONS" == "true" ]]; then
  need_cmd sqlx
fi

if [[ ! -f "$ROOT_DIR/.env" ]]; then
  echo "Missing .env in project root: $ROOT_DIR/.env"
  exit 1
fi

cleanup() {
  set +e
  if [[ -n "${BACKEND_PID:-}" ]]; then kill "$BACKEND_PID" >/dev/null 2>&1 || true; fi
  if [[ -n "${FRONTEND_PID:-}" ]]; then kill "$FRONTEND_PID" >/dev/null 2>&1 || true; fi
  wait >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

echo "==> Preparing frontend dependencies"
cd "$FRONTEND_DIR"
npm install

if [[ "$RUN_TESTS" == "true" ]]; then
  echo "==> Running backend tests"
  cd "$BACKEND_DIR"
  cargo test

  echo "==> Running frontend build"
  cd "$FRONTEND_DIR"
  npm run build
fi

if [[ "$RUN_MIGRATIONS" == "true" ]]; then
  echo "==> Running MySQL migrations"
  cd "$BACKEND_DIR"
  set -a
  source "$ROOT_DIR/.env"
  set +a
  sqlx migrate run
fi

echo "==> Starting backend on http://localhost:8080"
cd "$BACKEND_DIR"
set -a
source "$ROOT_DIR/.env"
set +a
cargo run &
BACKEND_PID=$!

echo "==> Waiting backend healthcheck"
for _ in $(seq 1 60); do
  if curl -fsS "http://localhost:8080/health" >/dev/null 2>&1; then
    echo "==> Backend is ready"
    break
  fi
  sleep 1
done

if ! curl -fsS "http://localhost:8080/health" >/dev/null 2>&1; then
  echo "Backend did not become ready in time. Check backend logs above."
  exit 1
fi

echo "==> Starting frontend on http://localhost:5173"
cd "$FRONTEND_DIR"
npm run dev -- --host &
FRONTEND_PID=$!

echo "==> Running. Press Ctrl+C to stop both services."
wait -n "$BACKEND_PID" "$FRONTEND_PID"
