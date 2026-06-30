# on-the-wall

A small Axum + SQLx (Postgres) service for posting to user "walls".

- **`server/`** — the Rust web service (Axum + SQLx).
- **`server/migrations/`** — SQL migrations, applied automatically on startup.
- **`server/.sqlx/`** — committed offline query cache (see [SQLx offline mode](#sqlx-offline-mode)).
- **`render.yaml`** — Render deployment config (web service + managed Postgres).

## Prerequisites

- Rust (stable) with `cargo`
- [`sqlx-cli`](https://crates.io/crates/sqlx-cli): `cargo install sqlx-cli --no-default-features --features rustls,postgres`
- Docker or Podman (for a local Postgres)

## Local development

### 1. Start Postgres

```bash
docker compose up -d        # or: podman compose up -d
```

This brings up Postgres on `localhost:5432` with database `db`, user `user`, password `password`.

### 2. Configure the environment

`server/.env` holds the local connection string (gitignored). It should contain:

```dotenv
DATABASE_URL=postgres://user:password@localhost/db
```

### 3. Apply migrations

```bash
cd server
cargo sqlx migrate run
```

The app **also** runs pending migrations automatically on startup (`sqlx::migrate!` in
`src/main.rs`), so this is mainly to get your local DB schema in place before generating
the query cache.

### 4. Run the server

```bash
cd server
cargo run
# or, with live reload:
cargo watch -x run
```

It listens on `$PORT` (default `3000` locally).

## Migrations

Migrations live in `server/migrations/` as timestamped SQL files. Create a new one with:

```bash
cd server
cargo sqlx migrate add <name>     # creates server/migrations/<timestamp>_<name>.sql
```

Edit the generated file, then apply it locally with `cargo sqlx migrate run`. Migrations
are applied automatically on app startup (idempotent — SQLx records applied migrations in
the `_sqlx_migrations` table), so production picks them up on the next deploy.

## SQLx offline mode

The `query!` macros are type-checked against a real database **at compile time**. To avoid
needing a database connection during the build (e.g. on Render), we commit an offline query
cache in `server/.sqlx/` and build with `SQLX_OFFLINE=true`.

**The cache is keyed on the SQL text of each query.** Changing a query (or changing schema
in a way that affects a query's result types) means the cache must be regenerated.

### When you change a query or its underlying schema

```bash
cd server
cargo sqlx migrate run     # 1. apply any new schema locally
cargo sqlx prepare         # 2. regenerate server/.sqlx/ from the query! macros
git add migrations/ .sqlx/ src/   # 3. commit them together
```

> ⚠️ **`server/.sqlx/` must be committed.** Without it, an offline build has nothing to
> check the `query!` macros against and will fail.

### Safety notes

- If you edit a query's SQL but forget to re-prepare, the offline build **fails loudly**
  with `run cargo sqlx prepare` — it won't ship a stale cache.
- The one case SQLx can't detect automatically is a **schema change with unchanged query
  text** (the cache is found by hash and reused). Guard against this in CI with a
  migrated DB:
  ```bash
  cargo sqlx prepare --check    # fails if .sqlx is out of date
  ```

## Deployment (Render)

Deployment is defined in `render.yaml`:

- **`wall-server`** (web service, Rust runtime)
  - `buildCommand`: `cd server && cargo build --release`
  - `startCommand`: `cd server && ./target/release/server`
  - `DATABASE_URL` injected from the managed database via `fromDatabase`
  - `SQLX_OFFLINE=true` so the build uses the committed `.sqlx/` cache (no DB needed at build time)
- **`wall-db`** — a managed Postgres instance

On deploy, Render builds the release binary against the offline cache, then the service
starts, runs any pending migrations against `wall-db`, and begins serving on `$PORT`.

### Deploy checklist

1. Commit your code **and** an up-to-date `server/.sqlx/` cache.
2. Push to the branch Render is watching.
3. Render builds (offline, DB-free), then boots the service.
4. On boot the app runs migrations automatically — no manual migration step required.
