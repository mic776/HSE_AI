CREATE TABLE IF NOT EXISTS teachers (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  login VARCHAR(64) NOT NULL UNIQUE,
  password_hash VARCHAR(255) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  updated_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3)
);

CREATE TABLE IF NOT EXISTS teacher_sessions (
  id CHAR(36) PRIMARY KEY,
  teacher_id BIGINT NOT NULL,
  csrf_token CHAR(36) NOT NULL,
  expires_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  INDEX idx_teacher_sessions_teacher_id (teacher_id),
  INDEX idx_teacher_sessions_expires_at (expires_at),
  CONSTRAINT fk_teacher_sessions_teacher FOREIGN KEY (teacher_id) REFERENCES teachers(id)
);

CREATE TABLE IF NOT EXISTS quizzes (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  owner_teacher_id BIGINT NOT NULL,
  title VARCHAR(255) NOT NULL,
  description TEXT NULL,
  is_published BOOLEAN NOT NULL DEFAULT FALSE,
  source_quiz_id BIGINT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  updated_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
  INDEX idx_quizzes_owner (owner_teacher_id),
  INDEX idx_quizzes_published (is_published),
  INDEX idx_quizzes_source (source_quiz_id),
  CONSTRAINT fk_quizzes_owner FOREIGN KEY (owner_teacher_id) REFERENCES teachers(id),
  CONSTRAINT fk_quizzes_source FOREIGN KEY (source_quiz_id) REFERENCES quizzes(id)
);

CREATE TABLE IF NOT EXISTS quiz_questions (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  quiz_id BIGINT NOT NULL,
  external_id VARCHAR(64) NOT NULL,
  q_type ENUM('open', 'single', 'multi') NOT NULL,
  prompt TEXT NOT NULL,
  position INT NOT NULL,
  UNIQUE KEY uk_quiz_questions_external (quiz_id, external_id),
  INDEX idx_quiz_questions_position (quiz_id, position),
  CONSTRAINT fk_quiz_questions_quiz FOREIGN KEY (quiz_id) REFERENCES quizzes(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS quiz_options (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  question_id BIGINT NOT NULL,
  external_id VARCHAR(64) NOT NULL,
  text TEXT NOT NULL,
  position INT NOT NULL,
  UNIQUE KEY uk_quiz_options_external (question_id, external_id),
  INDEX idx_quiz_options_position (question_id, position),
  CONSTRAINT fk_quiz_options_question FOREIGN KEY (question_id) REFERENCES quiz_questions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS quiz_answers (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  question_id BIGINT NOT NULL UNIQUE,
  open_text TEXT NULL,
  single_option_external_id VARCHAR(64) NULL,
  multi_option_external_ids JSON NULL,
  CONSTRAINT fk_quiz_answers_question FOREIGN KEY (question_id) REFERENCES quiz_questions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS quiz_publications (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  quiz_id BIGINT NOT NULL UNIQUE,
  published_by_teacher_id BIGINT NOT NULL,
  published_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  CONSTRAINT fk_quiz_publications_quiz FOREIGN KEY (quiz_id) REFERENCES quizzes(id) ON DELETE CASCADE,
  CONSTRAINT fk_quiz_publications_teacher FOREIGN KEY (published_by_teacher_id) REFERENCES teachers(id)
);

CREATE TABLE IF NOT EXISTS game_sessions (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  room_code VARCHAR(12) NOT NULL UNIQUE,
  join_token CHAR(36) NOT NULL UNIQUE,
  quiz_id BIGINT NOT NULL,
  teacher_id BIGINT NOT NULL,
  status ENUM('waiting', 'active', 'finished') NOT NULL,
  game_mode ENUM('platformer', 'shooter', 'tycoon') NOT NULL,
  started_at DATETIME(3) NULL,
  ended_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  INDEX idx_game_sessions_teacher_status (teacher_id, status),
  INDEX idx_game_sessions_quiz (quiz_id),
  CONSTRAINT fk_game_sessions_quiz FOREIGN KEY (quiz_id) REFERENCES quizzes(id),
  CONSTRAINT fk_game_sessions_teacher FOREIGN KEY (teacher_id) REFERENCES teachers(id)
);

CREATE TABLE IF NOT EXISTS session_participants (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  session_id BIGINT NOT NULL,
  nickname VARCHAR(64) NOT NULL,
  join_state ENUM('waiting', 'playing', 'left') NOT NULL,
  connected_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  left_at DATETIME(3) NULL,
  UNIQUE KEY uk_session_participants (session_id, nickname),
  INDEX idx_session_participants_state (session_id, join_state),
  CONSTRAINT fk_session_participants_session FOREIGN KEY (session_id) REFERENCES game_sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS session_question_states (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  session_id BIGINT NOT NULL,
  participant_id BIGINT NOT NULL,
  question_id BIGINT NOT NULL,
  attempts INT NOT NULL DEFAULT 0,
  is_correct BOOLEAN NOT NULL DEFAULT FALSE,
  first_attempt_at DATETIME(3) NULL,
  last_attempt_at DATETIME(3) NULL,
  UNIQUE KEY uk_session_question_states (session_id, participant_id, question_id),
  CONSTRAINT fk_session_question_states_session FOREIGN KEY (session_id) REFERENCES game_sessions(id) ON DELETE CASCADE,
  CONSTRAINT fk_session_question_states_participant FOREIGN KEY (participant_id) REFERENCES session_participants(id) ON DELETE CASCADE,
  CONSTRAINT fk_session_question_states_question FOREIGN KEY (question_id) REFERENCES quiz_questions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS session_answers (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  session_id BIGINT NOT NULL,
  participant_id BIGINT NOT NULL,
  question_id BIGINT NOT NULL,
  attempt_no INT NOT NULL,
  answer_payload JSON NOT NULL,
  is_correct BOOLEAN NOT NULL,
  answered_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  UNIQUE KEY uk_session_answers_attempt (session_id, participant_id, question_id, attempt_no),
  INDEX idx_session_answers_participant (session_id, participant_id),
  INDEX idx_session_answers_question (session_id, question_id),
  CONSTRAINT fk_session_answers_session FOREIGN KEY (session_id) REFERENCES game_sessions(id) ON DELETE CASCADE,
  CONSTRAINT fk_session_answers_participant FOREIGN KEY (participant_id) REFERENCES session_participants(id) ON DELETE CASCADE,
  CONSTRAINT fk_session_answers_question FOREIGN KEY (question_id) REFERENCES quiz_questions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS session_stats_aggregate (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  session_id BIGINT NOT NULL,
  participant_id BIGINT NULL,
  correct_count INT NOT NULL DEFAULT 0,
  wrong_count INT NOT NULL DEFAULT 0,
  correct_pct DECIMAL(5,2) NOT NULL DEFAULT 0,
  updated_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uk_session_stats_aggregate (session_id, participant_id),
  CONSTRAINT fk_session_stats_aggregate_session FOREIGN KEY (session_id) REFERENCES game_sessions(id) ON DELETE CASCADE,
  CONSTRAINT fk_session_stats_aggregate_participant FOREIGN KEY (participant_id) REFERENCES session_participants(id) ON DELETE CASCADE
);
