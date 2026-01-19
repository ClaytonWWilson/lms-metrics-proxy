CREATE TABLE IF NOT EXISTS requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Request identifiers
    endpoint TEXT NOT NULL,
    model TEXT NOT NULL,

    -- Timing information
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,

    -- Token metrics
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,

    -- Content
    prompt TEXT NOT NULL,
    output TEXT NOT NULL,

    -- Request metadata
    request_id TEXT,

    -- Error tracking
    is_error BOOLEAN DEFAULT 0,
    error_message TEXT,
    http_status INTEGER NOT NULL,

    -- Streaming metadata
    was_streamed BOOLEAN DEFAULT 0,

    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_model ON requests(model);
CREATE INDEX IF NOT EXISTS idx_endpoint ON requests(endpoint);
CREATE INDEX IF NOT EXISTS idx_start_time ON requests(start_time);
CREATE INDEX IF NOT EXISTS idx_is_error ON requests(is_error);
