-- Migration: 011_dedupe_system_plants.sql
-- Remove duplicatas das plantas de catálogo (Manjericão, Salsinha, Hortelã, Alecrim)
-- que foram criadas anteriormente por admins/usuários comuns, mantendo apenas
-- os registros oficiais semeados pelo usuário sistema (010_seed_system_plants).

DELETE FROM plants
WHERE LOWER(REPLACE(REPLACE(name, 'ã', 'a'), 'õ', 'o')) IN ('manjericao', 'salsinha', 'hortela', 'alecrim')
  AND created_by != '00000000-0000-0000-0000-000000000000';
