-- Marca como públicas todas as plantas já criadas por admins.
-- Necessário porque a migration 005 adicionou a coluna com DEFAULT 0,
-- deixando as plantas existentes como privadas incorretamente.
UPDATE plants SET publica = 1
WHERE created_by IN (
    SELECT id FROM users WHERE role = 'admin'
);