# 🌿 Mini Horta

Sistema completo de **monitoramento e irrigação automatizada de hortas** com integração IoT via Arduino, interface web em tempo real e controle remoto pelo celular.

Desenvolvido em **Rust** com Axum — escolha intencional para garantir segurança de memória, performance em tempo real e comunicação serial confiável com hardware físico.

---

## Funcionalidades

- 📡 **Leitura de sensores em tempo real** via Arduino (umidade do solo, luminosidade, estado da luz)
- 💧 **Irrigação automática** por threshold de umidade e **manual** pelo celular
- ☀️ **Controle de iluminação** remoto com timer diário de luz por planta
- 🌱 **Multi-planta** — cada usuário gerencia sua própria planta com dados individuais
- 🔒 **Plantas públicas e privadas** — plantas do catálogo visíveis a todos; plantas criadas por usuários são privadas (só o dono e admins veem)
- 🪴 **Catálogo de sistema** — Manjericão, Salsinha, Hortelã e Alecrim pré-cadastrados com valores de sensor calibrados
- 📊 **Dashboard ao vivo** com WebSocket — atualização a cada 5 segundos sem refresh
- 🔐 **Autenticação JWT** com dois níveis de acesso (Admin e Usuário)
- 🏡 **Sistema de hortas** — vínculo entre usuário, planta e dispositivo físico por código
- 🛠️ **Painel Admin** — listagem de todas as hortas e plantas (privadas identificadas com e-mail do dono), controle de luz e logs

---

## Stack

| Camada | Tecnologia |
|--------|-----------|
| Backend | Rust 1.75+ · Axum 0.7 · SQLx 0.7 |
| Banco de dados | SQLite |
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
| GET | `/plants` | AuthUser | Lista plantas públicas do catálogo + plantas privadas do próprio usuário |
| POST | `/plants` | Admin | Cadastra planta pública no catálogo |
| GET | `/plants/:id` | AuthUser | Detalhes da planta |
| PUT | `/plants/:id` | Admin | Edita planta |
| DELETE | `/plants/:id` | Admin | Remove planta |

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
| POST | `/hortas/connect` | AuthUser | Conecta horta por código; cria planta privada se não encontrar no catálogo |
| GET | `/hortas/mine` | AuthUser | Lista hortas do usuário |
| PATCH | `/hortas/:code/plant` | AuthUser | Troca planta ativa da horta |
| GET | `/hortas/:code/dashboard` | AuthUser | Dashboard completo da horta |
| POST | `/hortas/:code/regar` | AuthUser | Aciona irrigação pela horta |

### Admin
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| GET | `/admin/hortas` | Admin | Lista todas as hortas com planta e e-mail do dono |
| GET | `/admin/plants` | Admin | Lista todas as plantas; privadas exibem `[Privada – email]` na descrição |
| POST | `/admin/luz/:code/ligar` | Admin/Dono | Liga a luz (serial p/ Manjericão, simulado p/ demais) |
| POST | `/admin/luz/:code/desligar` | Admin/Dono | Desliga a luz |
| GET | `/admin/luz/:code/historico` | Admin/Dono | Histórico de luz (`?dias=7`, máx 30) |
| GET | `/admin/luz/:plant_id/logs` | AuthUser | Logs de acionamento de luz (`?limit=20`) |

### Sistema
| Método | Rota | Auth | Descrição |
|--------|------|------|-----------|
| GET | `/ws` | — | WebSocket — push de leituras em tempo real |
| GET | `/health` | — | Health check |

---

## Visibilidade de Plantas

O sistema diferencia dois tipos de planta:

**Públicas (`publica = 1`)** — cadastradas pelo usuário sistema ou por admins. Fazem parte do catálogo e são visíveis a todos os usuários. Atualmente: Manjericão, Salsinha, Hortelã, Alecrim.

**Privadas (`publica = 0`)** — criadas automaticamente quando um usuário conecta uma horta com um nome que não existe no catálogo. Só o próprio dono e admins as enxergam.

Ao conectar uma horta, o sistema busca **apenas no catálogo público** pelo nome normalizado (sem acentos, case-insensitive). Se não encontrar, cria uma instância privada para aquele usuário — nunca reutiliza a planta privada de outra pessoa.

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

> **Nota:** Atualmente apenas o Manjericão possui hardware físico (Arduino). As demais plantas (Salsinha, Hortelã, Alecrim e plantas privadas de usuários) operam em modo simulado — umidade e luz são calculadas por software com decaimento temporal e tokens aleatórios.

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
    ├── admin.rs       # /admin/* · controle de luz · painel admin
    ├── util.rs        # normalize_plant_name e helpers compartilhados
    └── ws.rs          # WebSocket · broadcast de leituras

migrations/
├── 001_initial_schema.sql      # Tabelas base: users, plants, sensor_readings
├── 002_hortas.sql              # Tabela hortas (vínculo user ↔ plant ↔ código)
├── 003_luz_historico.sql       # Histórico de períodos de luz
├── 004_luz_horas_dia.sql       # Limite diário de luz por planta
├── 005_plant_luz_status.sql    # Estado atual da luz por planta
├── 006_plant_umidade_status.sql# Umidade persistida para plantas simuladas
├── 007_luz_cooldown.sql        # Cooldown de 30s entre comandos de luz
├── 008_log_user_card.sql       # Coluna user_id em irrigation_logs
├── 009_plant_user.sql          # Coluna publica em plants (público/privado)
├── 010_seed_system_plants.sql  # Usuário sistema + plantas do catálogo
├── 011_dedupe_system_plants.sql# Remove duplicatas públicas do catálogo
├── 012_luz_logs.sql            # Tabela luz_logs + token de simulação
├── 013_add_is_system_flag.sql  # Flag de planta de sistema
└── 014_hortas_unique_owner_plant.sql # Restrição: um usuário por planta/horta

minihorta/
└── minihorta.ino   # Firmware do Arduino (leitura de sensor + controle serial)
```
