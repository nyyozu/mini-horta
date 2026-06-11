-- Status atual de luz por planta.
-- Separado de sensor_readings para que cada planta tenha seu próprio estado,
-- independente de qual planta o Arduino está enviando dados.
CREATE TABLE IF NOT EXISTS plant_luz_status (
    plant_id   TEXT PRIMARY KEY,
    ligada     INTEGER NOT NULL DEFAULT 0,   -- 0 = desligada, 1 = ligada
    updated_at TEXT    NOT NULL
);
