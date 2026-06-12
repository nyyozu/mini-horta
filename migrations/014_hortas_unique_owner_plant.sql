-- Migration: 014_hortas_unique_owner_plant.sql
--
-- Impede que o mesmo usuário tenha duas hortas apontando para a mesma
-- planta (instância duplicada). Usuários diferentes podem ter hortas
-- com a mesma planta (plant_id) sem problema.

CREATE UNIQUE INDEX IF NOT EXISTS idx_hortas_owner_plant_unique
    ON hortas (owner_id, plant_id);
