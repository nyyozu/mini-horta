# 🌿 Mini Horta

Sistema completo de **monitoramento e irrigação automatizada de hortas** com integração IoT via Arduino, interface web em tempo real e controle remoto pelo celular.

Desenvolvido em **Rust** com Axum — escolha intencional para garantir segurança de memória, performance em tempo real e comunicação serial confiável com hardware físico.

---

## Funcionalidades

- 📡 **Leitura de sensores em tempo real** via Arduino (umidade do solo, luminosidade, estado da luz)
- 💧 **Irrigação automática** por threshold de umidade e **manual** pelo celular
- ☀️ **Controle de iluminação** remoto com timer diário de luz por planta
- 🌱 **Multi-planta** — cada usuário gerencia sua própria planta com dados individuais
- 📊 **Dashboard ao vivo** com WebSocket — atualização a cada 5 segundos sem refresh
- 🔐 **Autenticação JWT** com dois níveis de acesso (Admin e Usuário)
- 🏡 **Sistema de hortas** — vínculo entre usuário, planta e dispositivo físico por código

---

## Stack

| Camada | Tecnologia |
|--------|-----------|
| Backend | Rust 1.75+ · Axum · SQLx |
| Banco de dados | SQLite (dev) |
| Autenticação | JWT (jsonwebtoken · bcrypt) |
| Tempo real | WebSocket nativo (Axum) |
| Hardware | Arduino via porta serial (serialport) |
| Frontend | HTML · CSS · JavaScript (single file) |

---

## Pré-requisitos

- [Rust 1.75+](https://rustup.rs/) — `rustup update stable`
- SQLite instalado
- Arduino com firmware enviando JSON via serial
- `sqlx-cli` para migrations:

```bash
cargo install sqlx-cli --no-default-features --features sqlite
```

---

## Configuração

```bash
# Clone o repositório
git clone <repo>
cd mini-horta

# Configure as variáveis de ambiente
cp .env.example .env
```

Edite o `.env`:

```env
DATABASE_URL=sqlite:./horta.db
JWT_SECRET=sua_chave_secreta_aqui
SERIAL_PORT=/dev/ttyUSB0       # Windows: COM3
SERIAL_BAUD=9600
BIND_ADDR=0.0.0.0:3000
FRONTEND_URL=http://localhost:5173
```

---

## Rodando

```bash
# Cria o banco e aplica todas as migrations
sqlx database create
sqlx migrate run

# Inicia o servidor
cargo run

# Ou com recompilação automática (requer cargo-watch)
cargo watch -x run
```

Servidor disponível em **http://localhost:3000**

---

## Endpoints

### Autenticação
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| POST | `/auth/register` | — | Cadastra usuário |
| POST | `/auth/login` | — | Login, retorna JWT |

### Plantas
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| GET | `/plants` | AuthUser | Lista plantas públicas + próprias |
| POST | `/plants` | Admin | Cadastra planta pública |
| GET | `/plants/:id` | AuthUser | Detalhes da planta |
| PUT | `/plants/:id` | Admin | Edita planta |

### Sensores
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| GET | `/sensors/:plant_id/latest` | AuthUser | Última leitura do sensor |
| GET | `/sensors/:plant_id/history` | AuthUser | Histórico (`?limit=50`) |

### Irrigação
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| POST | `/irrigation/manual` | AuthUser | Aciona irrigação manual |
| GET | `/irrigation/:plant_id/logs` | AuthUser | Logs de irrigação |

### Hortas
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| POST | `/hortas/connect` | AuthUser | Conecta horta por código |
| GET | `/hortas/mine` | AuthUser | Lista hortas do usuário |
| PATCH | `/hortas/:code/plant` | AuthUser | Troca planta ativa da horta |
| GET | `/hortas/:code/dashboard` | AuthUser | Dashboard completo da horta |
| POST | `/hortas/:code/regar` | AuthUser | Aciona irrigação pela horta |

### Controle (Admin)
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| POST | `/admin/luz/:code/ligar` | Admin | Liga a luz via serial |
| POST | `/admin/luz/:code/desligar` | Admin | Desliga a luz via serial |

### Sistema
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| GET | `/ws` | — | WebSocket — push em tempo real |
| GET | `/health` | — | Health check |

---

## Formato Arduino → Serial

O firmware do Arduino envia uma linha JSON por ciclo de leitura (padrão a cada 10s):

```json
{"plant_name":"Manjericao","humidity":65.3,"light_lux":3600.0,"luz_ligada":1}
```

| Campo | Tipo | Descrição |
|-------|------|-----------|
| `plant_name` | string | Nome da planta cadastrada no sistema |
| `humidity` | float | Umidade do solo (0–100%) |
| `light_lux` | float | Segundos de luz acumulados no dia |
| `luz_ligada` | int | Estado da luz: `1` = ligada, `0` = desligada |

O servidor aceita também comandos de volta ao Arduino:
- `LUZ_ON` / `LUZ_OFF` — controle de iluminação
- `IRRIGAR <segundos>` — aciona a bomba d'água

---

## Estrutura do projeto

```
src/
├── main.rs           # Inicialização, router, CORS, daemon serial
├── state.rs          # AppState compartilhado entre handlers
├── errors.rs         # AppError → respostas HTTP padronizadas
├── models.rs         # Structs de domínio (Plant, User, SensorReading…)
├── db.rs             # Queries SQLx — todas as operações de banco
├── auth.rs           # JWT · criação de token · extractors Axum
├── serial.rs         # Daemon de leitura serial + processamento de payload
└── routes/
    ├── mod.rs         # Router builder — monta todas as rotas
    ├── auth.rs        # /auth/register · /auth/login
    ├── plants.rs      # /plants/*
    ├── sensors.rs     # /sensors/*
    ├── irrigation.rs  # /irrigation/*
    ├── hortas.rs      # /hortas/* · dashboard · regar
    ├── dashboard.rs   # DashboardResponse · cálculo de status
    ├── admin.rs       # /admin/luz/* · controle serial
    └── ws.rs          # WebSocket · broadcast de leituras

migrations/
├── 001_initial_schema.sql
├── 002_hortas.sql
├── 003_luz_historico.sql
├── 004_luz_horas_dia.sql
├── 005_plant_luz_status.sql
├── 006_plant_umidade_status.sql
├── 007_luz_cooldown.sql
├── 008_log_user_card.sql
├── 009_plant_user.sql
└── RODAR DEPOIS DE CRIAR PLANTAS.sql
```
