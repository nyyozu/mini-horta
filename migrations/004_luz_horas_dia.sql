-- Migration: 004_luz_horas_dia.sql
-- Adiciona limite diário de luz (em horas) por planta.
-- Padrão 12h para não quebrar plantas já cadastradas.

ALTER TABLE plants ADD COLUMN luz_horas_dia REAL NOT NULL DEFAULT 12.0
    CHECK (luz_horas_dia > 0 AND luz_horas_dia <= 24);
