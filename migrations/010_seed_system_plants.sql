-- Migration: 010_seed_system_plants.sql
-- Cria um usuário "sistema" fixo (não recriado a cada boot, role admin)
-- e semeia as plantas padrão do catálogo (Manjericão, Salsinha, Hortelã, Alecrim)
-- como públicas, com os valores usados pelo firmware do Arduino + benefícios.
--
-- Usar created_by = SYSTEM_USER_ID garante que essas plantas:
--  - independem do admin recriado a cada restart do banco
--  - só podem ser editadas por qualquer usuário com role 'admin'
--    (checado via AdminUser extractor, não por ownership)
--  - não são removidas em cascata, pois o usuário sistema nunca é deletado

INSERT OR IGNORE INTO users (id, email, password_hash, role, created_at)
VALUES (
    '00000000-0000-0000-0000-000000000000',
    'sistema@horta.local',
    -- hash bcrypt de senha aleatória inutilizável; este usuário não faz login
    '$2b$12$0000000000000000000000000000000000000000000000000000',
    'admin',
    '2026-01-01T00:00:00Z'
);

INSERT OR IGNORE INTO plants (id, name, description, humidity_min, humidity_max, luz_horas_dia, created_by, created_at, publica)
VALUES
(
    '00000000-0000-0000-0000-000000000001',
    'Manjericao',
    'Antibacteriano, repelente natural de insetos, auxilia na digestão e é usado em chás calmantes.',
    60.0, 80.0, 12.0,
    '00000000-0000-0000-0000-000000000000',
    '2026-01-01T00:00:00Z',
    1
),
(
    '00000000-0000-0000-0000-000000000002',
    'Salsinha',
    'Rica em vitamina C e K, diurética, antioxidante e amplamente usada como tempero culinário.',
    65.0, 80.0, 12.0,
    '00000000-0000-0000-0000-000000000000',
    '2026-01-01T00:00:00Z',
    1
),
(
    '00000000-0000-0000-0000-000000000003',
    'Hortela',
    'Alivia náuseas e problemas digestivos, descongestionante natural, refrescante em chás e sucos.',
    70.0, 85.0, 8.0,
    '00000000-0000-0000-0000-000000000000',
    '2026-01-01T00:00:00Z',
    1
),
(
    '00000000-0000-0000-0000-000000000004',
    'Alecrim',
    'Estimula memória e concentração, anti-inflamatório, melhora a circulação e é usado em temperos e óleos essenciais.',
    40.0, 60.0, 12.0,
    '00000000-0000-0000-0000-000000000000',
    '2026-01-01T00:00:00Z',
    1
);
