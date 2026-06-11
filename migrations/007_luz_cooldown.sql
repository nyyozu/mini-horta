-- Cooldown de comandos de luz por horta.
-- Evita spam de ligar/desligar que poderia danificar o hardware físico.
-- Usuários comuns precisam aguardar 30s entre comandos no Manjericão.
-- Admins não passam por essa verificação.
CREATE TABLE IF NOT EXISTS luz_cooldown (
    horta_id           TEXT PRIMARY KEY REFERENCES hortas(id) ON DELETE CASCADE,
    ultimo_comando_at  INTEGER NOT NULL   -- Unix timestamp do último comando
);
