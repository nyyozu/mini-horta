-- Adiciona o usuário que disparou a irrigação manual.
-- NULL para registros automáticos (trigger = 'auto').
ALTER TABLE irrigation_logs ADD COLUMN triggered_by_user_id TEXT REFERENCES users(id);
