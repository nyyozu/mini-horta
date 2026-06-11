-- Plantas criadas pelo admin são públicas (visíveis a todos).
-- Plantas criadas pelo usuário comum ao conectar uma horta são privadas (só ele vê).
ALTER TABLE plants ADD COLUMN publica INTEGER NOT NULL DEFAULT 0;

-- Plantas existentes criadas por admins já devem ser públicas.
-- Ajuste manual se necessário após rodar a migration:
-- UPDATE plants SET publica = 1 WHERE id IN (...);
