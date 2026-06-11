-- Status atual de umidade por planta.
-- Separado de sensor_readings para que cada planta tenha seu próprio valor,
-- independente de qual planta o Arduino está enviando dados.
-- Inicializado com (humidity_min + humidity_max) / 2 ao conectar a horta.
-- Atualizado a cada rega manual via POST /hortas/:code/regar.
CREATE TABLE IF NOT EXISTS plant_umidade_status (
    plant_id   TEXT PRIMARY KEY REFERENCES plants(id) ON DELETE CASCADE,
    umidade    REAL NOT NULL CHECK (umidade >= 0 AND umidade <= 100),
    updated_at TEXT NOT NULL
);
