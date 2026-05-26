-- Migration: 001_initial_schema.sql
-- Criação das tabelas principais do sistema Mini Horta

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,               -- UUID v4 como string
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'user'    -- 'admin' | 'user'
                  CHECK (role IN ('admin', 'user')),
    created_at    TEXT NOT NULL                   -- ISO 8601 UTC
);

CREATE TABLE IF NOT EXISTS plants (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    description   TEXT,
    humidity_min  REAL NOT NULL CHECK (humidity_min >= 0 AND humidity_min <= 100),
    humidity_max  REAL NOT NULL CHECK (humidity_max >= 0 AND humidity_max <= 100),
    created_by    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at    TEXT NOT NULL,
    CHECK (humidity_min < humidity_max)
);

CREATE TABLE IF NOT EXISTS sensor_readings (
    id         TEXT PRIMARY KEY,
    plant_id   TEXT NOT NULL REFERENCES plants(id) ON DELETE CASCADE,
    humidity   REAL NOT NULL CHECK (humidity >= 0 AND humidity <= 100),
    light_lux  REAL NOT NULL CHECK (light_lux >= 0),
    read_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_readings_plant_time
    ON sensor_readings (plant_id, read_at DESC);

CREATE TABLE IF NOT EXISTS irrigation_logs (
    id            TEXT PRIMARY KEY,
    plant_id      TEXT NOT NULL REFERENCES plants(id) ON DELETE CASCADE,
    triggered_by  TEXT NOT NULL CHECK (triggered_by IN ('auto', 'manual')),
    duration_sec  INTEGER NOT NULL CHECK (duration_sec > 0),
    started_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_irrigation_plant_time
    ON irrigation_logs (plant_id, started_at DESC);
