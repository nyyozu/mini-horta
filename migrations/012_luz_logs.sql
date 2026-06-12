-- Migration: 012_luz_logs.sql
-- Cria tabela de logs de ações de luz (espelha irrigation_logs),
-- e adiciona coluna de "token" aleatório em luz_historico para
-- plantas que não recebem comando serial real (não-Manjericão).

CREATE TABLE IF NOT EXISTS luz_logs (
    id                    TEXT PRIMARY KEY,
    plant_id              TEXT NOT NULL REFERENCES plants(id) ON DELETE CASCADE,
    acao                  TEXT NOT NULL CHECK (acao IN ('ligar', 'desligar')),
    token                 REAL,                          -- valor aleatório (simulação), NULL para Manjericão
    triggered_by_user_id  TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at            TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_luz_logs_plant_time
    ON luz_logs (plant_id, created_at DESC);
