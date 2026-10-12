ALTER TABLE card_ai_messages
ADD COLUMN reasoning_content TEXT NOT NULL DEFAULT '';

ALTER TABLE card_ai_messages
ADD COLUMN web_search_enabled INTEGER NOT NULL DEFAULT 0
CHECK (web_search_enabled IN (0, 1));

INSERT INTO schema_migrations (version, name, applied_at)
VALUES (3, 'ai_response_details', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
