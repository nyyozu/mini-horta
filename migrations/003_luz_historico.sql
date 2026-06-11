-- migrations/003_luz_historico.sql

-- Adiciona coluna luz_ligada na tabela de leituras
-- (0 = desligada, 1 = ligada no momento da leitura)
ALTER TABLE sensor_readings ADD COLUMN luz_ligada INTEGER NOT NULL DEFAULT 0;

-- Histórico diário de luz por planta
-- Cada linha = um período em que a luz ficou ligada
CREATE TABLE IF NOT EXISTS luz_historico (
    id          TEXT PRIMARY KEY,
    plant_id    TEXT NOT NULL REFERENCES plants(id) ON DELETE CASCADE,
    ligou_em    TEXT NOT NULL,   -- ISO 8601 — quando a luz foi ligada
    desligou_em TEXT,            -- NULL enquanto ainda estiver ligada
    duracao_sec INTEGER          -- preenchido ao desligar
);

CREATE INDEX IF NOT EXISTS idx_luz_plant_dia
    ON luz_historico (plant_id, ligou_em DESC);
