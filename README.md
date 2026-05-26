# Mini Horta — Back-end Rust

Sistema de monitoramento e irrigação automática de horta, com leitura de sensores via Arduino e interface web.

## Pré-requisitos

- Rust 1.75+ (`rustup update stable`)
- SQLite (desenvolvimento) ou PostgreSQL (produção)
- Arduino com firmware que envia JSON via serial

## Configuração

```bash
# Clone e entre no diretório
git clone <repo>
cd mini-horta

# Copie e edite as variáveis de ambiente
cp .env.example .env
# Edite .env: JWT_SECRET, SERIAL_PORT, DATABASE_URL

# Instale o sqlx-cli para gerenciar migrations
cargo install sqlx-cli --no-default-features --features sqlite
```

## Rodando

```bash
# Cria o banco e aplica migrations
sqlx database create
sqlx migrate run

# Inicia o servidor (com recompilação automática via cargo-watch)
cargo watch -x run

# Ou apenas:
cargo run
```

O servidor sobe em `http://localhost:3000`.

## Endpoints

| Método | Rota                              | Auth     | Descrição                    |
|--------|-----------------------------------|----------|------------------------------|
| POST   | `/auth/register`                  | —        | Cadastra usuário             |
| POST   | `/auth/login`                     | —        | Login, retorna JWT           |
| GET    | `/plants`                         | AuthUser | Lista plantas                |
| POST   | `/plants`                         | Admin    | Cadastra planta              |
| GET    | `/plants/:id`                     | AuthUser | Detalhes da planta           |
| GET    | `/sensors/:plant_id/latest`       | AuthUser | Última leitura               |
| GET    | `/sensors/:plant_id/history`      | AuthUser | Histórico (`?limit=50`)      |
| POST   | `/irrigation/manual`              | AuthUser | Aciona irrigação manual      |
| GET    | `/irrigation/:plant_id/logs`      | AuthUser | Logs de irrigação            |
| GET    | `/ws`                             | —        | WebSocket — push em tempo real|
| GET    | `/health`                         | —        | Health check                 |

## Formato Arduino → Serial

O firmware do Arduino deve enviar uma linha JSON por leitura:

```json
{"plant_id":"550e8400-e29b-41d4-a716-446655440000","humidity":65.3,"light_lux":1200.0}
```

## Estrutura do projeto

```
src/
├── main.rs        # Inicialização, router, daemon serial
├── state.rs       # AppState compartilhado
├── errors.rs      # AppError → respostas HTTP
├── models.rs      # Structs de domínio
├── db.rs          # Queries SQLx
├── auth.rs        # JWT + extractors Axum
├── serial.rs      # Daemon de leitura serial
└── routes/
    ├── mod.rs      # Router builder
    ├── auth.rs     # /auth/*
    ├── plants.rs   # /plants/*
    ├── sensors.rs  # /sensors/*
    ├── irrigation.rs # /irrigation/*
    └── ws.rs       # /ws WebSocket
migrations/
└── 001_initial_schema.sql
```
