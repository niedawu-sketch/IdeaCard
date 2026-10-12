CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL
);

CREATE TABLE cards (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  original_content TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  source_type TEXT NOT NULL CHECK (source_type IN ('personal','web','document','image','media')),
  capture_method TEXT NOT NULL CHECK (capture_method IN ('quick','clipboard','screenshot','file')),
  source_name TEXT NOT NULL DEFAULT '',
  source_uri TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL CHECK (status IN ('unprocessed','annotated','pending_analysis','concluded','archived')),
  content_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE attachments (
  id TEXT PRIMARY KEY,
  card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
  kind TEXT NOT NULL CHECK (kind IN ('document','image','audio','video')),
  original_name TEXT NOT NULL,
  relative_path TEXT NOT NULL UNIQUE,
  mime_type TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  checksum TEXT NOT NULL,
  extraction_status TEXT NOT NULL CHECK (extraction_status IN ('pending','completed','empty','failed','not_applicable')),
  extraction_error TEXT NOT NULL DEFAULT '',
  extracted_text TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE TABLE analysis_sessions (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  question TEXT NOT NULL,
  web_search_enabled INTEGER NOT NULL CHECK (web_search_enabled IN (0, 1)),
  status TEXT NOT NULL CHECK (status IN ('draft','running','completed','failed','cancelled')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE analysis_session_cards (
  analysis_session_id TEXT NOT NULL REFERENCES analysis_sessions(id) ON DELETE RESTRICT,
  card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (analysis_session_id, card_id),
  UNIQUE (analysis_session_id, position)
);

CREATE TABLE analysis_messages (
  id TEXT PRIMARY KEY,
  analysis_session_id TEXT NOT NULL REFERENCES analysis_sessions(id) ON DELETE RESTRICT,
  role TEXT NOT NULL CHECK (role IN ('user','assistant')),
  content TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status = 'completed'),
  created_at TEXT NOT NULL
);

CREATE TABLE message_card_citations (
  message_id TEXT NOT NULL REFERENCES analysis_messages(id) ON DELETE RESTRICT,
  card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
  label TEXT NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (message_id, card_id),
  UNIQUE (message_id, position)
);

CREATE TABLE message_web_citations (
  message_id TEXT NOT NULL REFERENCES analysis_messages(id) ON DELETE RESTRICT,
  url TEXT NOT NULL,
  title TEXT NOT NULL,
  start_index INTEGER NOT NULL CHECK (start_index >= 0),
  end_index INTEGER NOT NULL CHECK (end_index >= start_index),
  PRIMARY KEY (message_id, url, start_index)
);

CREATE TABLE insights (
  id TEXT PRIMARY KEY,
  analysis_session_id TEXT NOT NULL REFERENCES analysis_sessions(id) ON DELETE RESTRICT,
  saved_through_message_id TEXT NOT NULL REFERENCES analysis_messages(id) ON DELETE RESTRICT,
  title TEXT NOT NULL,
  question TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE insight_messages (
  insight_id TEXT NOT NULL REFERENCES insights(id) ON DELETE RESTRICT,
  message_id TEXT NOT NULL REFERENCES analysis_messages(id) ON DELETE RESTRICT,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (insight_id, message_id),
  UNIQUE (insight_id, position)
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX cards_status_updated_at_idx ON cards(status, updated_at DESC);
CREATE INDEX cards_source_type_updated_at_idx ON cards(source_type, updated_at DESC);
CREATE INDEX cards_content_hash_idx ON cards(content_hash);
CREATE INDEX cards_source_uri_idx ON cards(source_uri) WHERE source_uri <> '';
CREATE INDEX attachments_card_id_idx ON attachments(card_id);
CREATE INDEX attachments_checksum_idx ON attachments(checksum);
CREATE INDEX analysis_session_cards_card_id_idx ON analysis_session_cards(card_id);
CREATE INDEX analysis_messages_session_id_idx ON analysis_messages(analysis_session_id);
CREATE INDEX message_card_citations_card_id_idx ON message_card_citations(card_id);
CREATE INDEX message_web_citations_message_id_idx ON message_web_citations(message_id);
CREATE INDEX insights_session_id_idx ON insights(analysis_session_id);
CREATE INDEX insights_saved_message_id_idx ON insights(saved_through_message_id);
CREATE INDEX insight_messages_message_id_idx ON insight_messages(message_id);

INSERT INTO schema_migrations (version, name, applied_at)
VALUES (1, 'initial', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
