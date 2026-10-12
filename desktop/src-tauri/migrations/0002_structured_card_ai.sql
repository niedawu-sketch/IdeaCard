CREATE TABLE card_documents (
  card_id TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE RESTRICT,
  document_json TEXT NOT NULL CHECK (length(document_json) > 0),
  schema_version INTEGER NOT NULL CHECK (schema_version = 1),
  updated_at TEXT NOT NULL
);

CREATE TABLE card_document_images (
  card_id TEXT NOT NULL REFERENCES card_documents(card_id) ON DELETE RESTRICT,
  attachment_id TEXT NOT NULL REFERENCES attachments(id) ON DELETE RESTRICT,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (card_id, attachment_id),
  UNIQUE (card_id, position)
);

CREATE TABLE concepts (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(trim(name)) > 0),
  normalized_name TEXT NOT NULL UNIQUE CHECK (length(normalized_name) > 0),
  explanation TEXT NOT NULL CHECK (length(trim(explanation)) > 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE concept_mentions (
  id TEXT PRIMARY KEY,
  concept_id TEXT NOT NULL REFERENCES concepts(id) ON DELETE RESTRICT,
  card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
  selected_text TEXT NOT NULL CHECK (length(trim(selected_text)) > 0),
  created_at TEXT NOT NULL
);

CREATE TABLE card_ai_threads (
  id TEXT PRIMARY KEY,
  card_id TEXT NOT NULL UNIQUE REFERENCES cards(id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE card_ai_messages (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES card_ai_threads(id) ON DELETE RESTRICT,
  role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
  content TEXT NOT NULL CHECK (length(trim(content)) > 0),
  created_at TEXT NOT NULL
);

CREATE TABLE card_ai_citations (
  message_id TEXT NOT NULL REFERENCES card_ai_messages(id) ON DELETE RESTRICT,
  url TEXT NOT NULL CHECK (length(trim(url)) > 0),
  title TEXT NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (message_id, url, position),
  UNIQUE (message_id, position)
);

CREATE INDEX card_document_images_attachment_id_idx
  ON card_document_images(attachment_id);
CREATE INDEX concepts_updated_at_idx
  ON concepts(updated_at DESC, id ASC);
CREATE INDEX concept_mentions_concept_id_idx
  ON concept_mentions(concept_id, created_at ASC);
CREATE INDEX concept_mentions_card_id_idx
  ON concept_mentions(card_id, created_at ASC);
CREATE INDEX card_ai_messages_thread_id_idx
  ON card_ai_messages(thread_id, created_at ASC);

INSERT INTO schema_migrations (version, name, applied_at)
VALUES (2, 'structured_card_ai', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
