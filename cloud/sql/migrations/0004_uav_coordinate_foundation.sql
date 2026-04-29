-- ============================================================
-- 0004: UAV Coordinate Foundation + Tree Registry
-- ============================================================

-- 1. Plantations (根节点，隔离不同作物)
CREATE TABLE IF NOT EXISTS plantations (
    id            SERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    crop_type     TEXT NOT NULL,  -- 'rice' | 'oil_palm' | 'durian' ...
    location_desc TEXT NOT NULL DEFAULT '',
    metadata_json JSONB NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. UAV Missions
CREATE TABLE IF NOT EXISTS uav_missions (
    id                SERIAL PRIMARY KEY,
    plantation_id     INTEGER NOT NULL REFERENCES plantations(id),
    mission_name      TEXT NOT NULL DEFAULT '',
    mission_type      TEXT NOT NULL DEFAULT 'initial_mapping',
    status            TEXT NOT NULL DEFAULT 'uploaded',
    coordinate_system TEXT NOT NULL DEFAULT 'local_plantation_v1',
    origin_description TEXT NOT NULL DEFAULT '',
    captured_at       TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 3. UAV Orthomosaics
CREATE TABLE IF NOT EXISTS uav_orthomosaics (
    id                SERIAL PRIMARY KEY,
    mission_id        INTEGER NOT NULL REFERENCES uav_missions(id),
    image_url         TEXT NOT NULL DEFAULT '',
    width             INTEGER NOT NULL DEFAULT 0,
    height            INTEGER NOT NULL DEFAULT 0,
    coordinate_system TEXT NOT NULL DEFAULT 'local_plantation_v1',
    origin_x          DOUBLE PRECISION NOT NULL DEFAULT 0,
    origin_y          DOUBLE PRECISION NOT NULL DEFAULT 0,
    resolution        DOUBLE PRECISION NOT NULL DEFAULT 0,
    transform_json    JSONB NOT NULL DEFAULT '{}',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 4. UAV Tiles
CREATE TABLE IF NOT EXISTS uav_tiles (
    id              SERIAL PRIMARY KEY,
    orthomosaic_id  INTEGER NOT NULL REFERENCES uav_orthomosaics(id),
    tile_url        TEXT NOT NULL DEFAULT '',
    tile_x          INTEGER NOT NULL DEFAULT 0,
    tile_y          INTEGER NOT NULL DEFAULT 0,
    tile_width      INTEGER NOT NULL DEFAULT 0,
    tile_height     INTEGER NOT NULL DEFAULT 0,
    global_offset_x INTEGER NOT NULL DEFAULT 0,
    global_offset_y INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 5. Trees (通用树木登记，species 区分作物)
CREATE TABLE IF NOT EXISTS trees (
    id                    SERIAL PRIMARY KEY,
    tree_code             TEXT UNIQUE NOT NULL,
    plantation_id         INTEGER NOT NULL REFERENCES plantations(id),
    species               TEXT NOT NULL DEFAULT 'oil_palm',
    block_id              TEXT,
    coordinate_x          DOUBLE PRECISION,
    coordinate_y          DOUBLE PRECISION,
    coordinate_system     TEXT NOT NULL DEFAULT 'local_plantation_v1',
    crown_center_x        DOUBLE PRECISION,
    crown_center_y        DOUBLE PRECISION,
    crown_bbox_json       JSONB,
    source_orthomosaic_id INTEGER REFERENCES uav_orthomosaics(id),
    manual_verified       BOOLEAN NOT NULL DEFAULT FALSE,
    barcode_value         TEXT,
    current_status        TEXT NOT NULL DEFAULT 'active',
    metadata_json         JSONB NOT NULL DEFAULT '{}',
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 6. UAV Tree Detections
CREATE TABLE IF NOT EXISTS uav_tree_detections (
    id              SERIAL PRIMARY KEY,
    mission_id      INTEGER NOT NULL REFERENCES uav_missions(id),
    orthomosaic_id  INTEGER REFERENCES uav_orthomosaics(id),
    tile_id         INTEGER REFERENCES uav_tiles(id),
    bbox_tile_json  JSONB,
    bbox_global_json JSONB,
    crown_center_x  DOUBLE PRECISION,
    crown_center_y  DOUBLE PRECISION,
    confidence      DOUBLE PRECISION NOT NULL DEFAULT 0,
    matched_tree_id INTEGER REFERENCES trees(id),
    review_status   TEXT NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 7. Tree Coordinate History
CREATE TABLE IF NOT EXISTS tree_coordinate_history (
    id               SERIAL PRIMARY KEY,
    tree_id          INTEGER NOT NULL REFERENCES trees(id),
    mission_id       INTEGER NOT NULL REFERENCES uav_missions(id),
    detected_x       DOUBLE PRECISION,
    detected_y       DOUBLE PRECISION,
    center_shift     DOUBLE PRECISION,
    crown_bbox_json  JSONB,
    match_confidence DOUBLE PRECISION,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_trees_plantation ON trees(plantation_id);
CREATE INDEX IF NOT EXISTS idx_trees_species ON trees(species);
CREATE INDEX IF NOT EXISTS idx_trees_code ON trees(tree_code);
CREATE INDEX IF NOT EXISTS idx_uav_det_mission ON uav_tree_detections(mission_id);
CREATE INDEX IF NOT EXISTS idx_uav_det_status ON uav_tree_detections(review_status);
CREATE INDEX IF NOT EXISTS idx_tree_coord_hist_tree ON tree_coordinate_history(tree_id);

-- Sequence for unique tree_code generation
CREATE SEQUENCE IF NOT EXISTS tree_code_seq;

-- 约束补充（幂等，不会重复添加）
DO $$ BEGIN
  -- trees
  ALTER TABLE trees ADD CONSTRAINT uq_trees_barcode UNIQUE (barcode_value);
  ALTER TABLE trees ADD CONSTRAINT chk_trees_status
    CHECK (current_status IN ('active','dead','removed','replanted'));

  -- uav_tree_detections
  ALTER TABLE uav_tree_detections ADD CONSTRAINT chk_det_confidence
    CHECK (confidence >= 0 AND confidence <= 1);
  ALTER TABLE uav_tree_detections ADD CONSTRAINT chk_det_review_status
    CHECK (review_status IN ('pending','confirmed','rejected','corrected'));
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
