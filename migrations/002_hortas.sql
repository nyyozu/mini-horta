-- migrations/002_hortas.sql

CREATE TABLE IF NOT EXISTS hortas (
    id         TEXT PRIMARY KEY,
    code       TEXT NOT NULL UNIQUE,   -- código que vem na caixinha (ex: "0101")
    plant_id   TEXT NOT NULL REFERENCES plants(id) ON DELETE CASCADE,
    owner_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_hortas_code    ON hortas (code);
CREATE INDEX IF NOT EXISTS idx_hortas_owner   ON hortas (owner_id);
