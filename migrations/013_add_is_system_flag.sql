-- Migration: 013_add_is_system_flag.sql
--
-- Identifica o usuário "sistema" por flag, em vez de por um ID fixo
-- e previsível. Necessário para o seed em runtime (ver db.rs).

ALTER TABLE users ADD COLUMN is_system INTEGER NOT NULL DEFAULT 0;
