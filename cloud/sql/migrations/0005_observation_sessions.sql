-- ============================================================
-- 0005: Observation Sessions + Session Images
-- ============================================================

CREATE TABLE IF NOT EXISTS observation_sessions (
    id            SERIAL PRIMARY KEY,
    tree_id       INTEGER NOT NULL REFERENCES trees(id),
    session_code  TEXT UNIQUE NOT NULL,
    status        TEXT NOT NULL DEFAULT 'active',
    metadata_json JSONB NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS session_images (
    id                 SERIAL PRIMARY KEY,
    session_id         INTEGER NOT NULL REFERENCES observation_sessions(id),
    image_url          TEXT NOT NULL,
    image_role         TEXT NOT NULL,
    upload_id          TEXT,
    mock_analysis_json JSONB NOT NULL DEFAULT '{}',
    metadata_json      JSONB NOT NULL DEFAULT '{}',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_observation_sessions_tree ON observation_sessions(tree_id);
CREATE INDEX IF NOT EXISTS idx_session_images_session ON session_images(session_id);
CREATE INDEX IF NOT EXISTS idx_session_images_role ON session_images(image_role);

CREATE SEQUENCE IF NOT EXISTS observation_session_code_seq;

DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_observation_sessions_status') THEN
    ALTER TABLE observation_sessions ADD CONSTRAINT chk_observation_sessions_status
      CHECK (status IN ('active','complete','abandoned'));
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_session_images_role') THEN
    ALTER TABLE session_images ADD CONSTRAINT chk_session_images_role
      CHECK (image_role IN ('fruit','trunk_base','crown'));
  END IF;
END $$;
