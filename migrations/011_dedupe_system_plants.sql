-- Migration: 011_dedupe_system_plants.sql (REESCRITA)
--
-- A versão anterior desta migration removia plantas "duplicadas" globalmente
-- (por nome normalizado), o que viola o requisito do produto:
--   - Usuários diferentes PODEM ter plantas com o mesmo nome.
--   - O mesmo usuário NÃO PODE ter plantas duplicadas com o mesmo nome.
--
-- Esta versão não deleta nenhuma linha. Apenas garante unicidade por
-- usuário (created_by + nome normalizado em minúsculas).

CREATE UNIQUE INDEX IF NOT EXISTS idx_plants_owner_name_unique
    ON plants (created_by, LOWER(name));
