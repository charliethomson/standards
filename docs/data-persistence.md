# Data & persistence

## Rule

Services persist with **`sqlx` + SQLite**, one file per service under the data dir, with
**`.sql` migrations** run on boot. Entities are keyed by a **typed `Id<T>` (UUIDv7)**. The
central **auth** service is the documented exception: it uses **SeaORM + MariaDB** because
its data is shared and relational.

## Typed identifiers

`Id<T>` is a UUIDv7 tagged with the entity it identifies, so an `Id<Order>` can't be
passed where an `Id<Item>` is expected. UUIDv7 is time-ordered, so primary-key inserts stay
append-friendly and sorting by id approximates creation order.

```rust
pub struct Id<T> {
    raw: Uuid,                       // Uuid::now_v7()
    _marker: PhantomData<fn() -> T>, // zero-cost type tag
}
```

Serializes as a canonical hyphenated UUID string in JSON. `Copy + Clone + Hash + Ord`. Use it
for every entity id in `core`. Template: [`templates/rust/id.rs`](../templates/rust/id.rs).

`Id<T>` is the *internal* identity. Entities that appear in user-facing URLs also carry a
**public short id** (a stored Crockford-Base32 alias); the UUID never leaves internal/admin
surfaces. See [public-ids.md](public-ids.md).

## Connection & pool

```rust
let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
    .create_if_missing(true)
    .foreign_keys(true)                 // enforce FKs (off by default in SQLite)
    .busy_timeout(Duration::from_secs(5))
    .log_statements(log::LevelFilter::Debug)
    .log_slow_statements(log::LevelFilter::Warn, Duration::from_millis(200));

let pool = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
```

The `Db` handle owns the pool **and** the data dir (file assets live on disk, metadata in the
DB). Open it once at startup; pass it through `AppState`.

## Migrations

`.sql` files in `db/migrations/`, embedded and run on connect:

```rust
sqlx::migrate!("./migrations").run(&pool).await?;
```

Forward-only, checked in. No ORM-generated schema (except auth/SeaORM). Run migrations as part
of `Db::connect`, before serving.

## Repositories

Queries live as methods on `Db` in the `db` crate (one module per entity), using `sqlx::query`
with bound parameters. **Classify write failures** into domain errors so the API maps them to
the right status:

```rust
pub(crate) fn map_write_err(e: &sqlx::Error, conflict: &str, fk: &str) -> Error {
    let msg = e.to_string();
    if msg.contains("UNIQUE constraint failed")      { Error::Conflict(conflict.into()) }
    else if msg.contains("FOREIGN KEY constraint failed") { Error::invalid(fk) }
    else                                             { Error::storage(format!("write: {e}")) }
}
```

See [error-handling.md](error-handling.md) for the full error model.

## The auth exception (SeaORM + MariaDB)

Auth uses **SeaORM 2.x against MariaDB**: entities + a dedicated **migration crate**
(`MigratorTrait`) and a **seed binary** that inserts the admin user, default app, and grants.
Reach for this only when the data is genuinely shared/relational across services; a normal
product service stays on sqlx/SQLite.

## Pluggable storage (when needed)

A service whose backend legitimately varies (e.g. someproduct's `KeeperBackend`) defines an
`#[async_trait]` storage trait in a `common` crate with `InMemory` / `FileSystem` / `Sql`
implementations selected by feature. Don't reach for this unless the variability is real.

## Checklist

- [ ] `sqlx` + SQLite; one DB file under the data dir; pool opened once in `Db::connect`.
- [ ] `create_if_missing`, `foreign_keys`, `busy_timeout`, slow-statement logging set.
- [ ] `.sql` migrations in `db/migrations/`, run via `sqlx::migrate!` on boot.
- [ ] Every entity id is a typed `Id<T>` (UUIDv7).
- [ ] Entities exposed in user-facing URLs also have a public short id ([public-ids.md](public-ids.md)).
- [ ] Write errors classified (UNIQUE→Conflict, FK→Invalid) into domain errors.
- [ ] SeaORM/MariaDB only for shared relational data (auth); pluggable backends only when real.
