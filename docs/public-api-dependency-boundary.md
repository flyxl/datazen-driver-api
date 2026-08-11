# Driver API Public Dependency Boundary

## Purpose

`datazen-driver-api` is the compile-time contract between DataZen and database driver plugins. The API must remain independent of any specific database implementation so that different drivers can use different database libraries and library versions without creating type or ABI coupling between plugins.

## Public API rule

A type may cross the Driver API boundary only if it is one of the following:

- a Rust primitive or standard-library collection/type;
- a type defined by `datazen-driver-api` itself;
- `serde` serialization traits/attributes where required by the API;
- `serde_json::Value` or other `serde_json` data-model types used for transport-neutral JSON data.

Public Driver API signatures and public structs/enums **must not expose implementation-specific types**.

## Forbidden implementation dependencies

The following categories must not appear in public traits, function signatures, public struct fields, enum variants, or public type aliases:

- `sqlx` types (`Pool`, `Row`, `Transaction`, `Error`, database-specific types, etc.);
- `tokio` runtime or synchronization types;
- database-specific crates such as `mongodb`, `redis`, `clickhouse`, `mysql_async`, `tokio-postgres`, and similar libraries;
- HTTP client implementation types such as `reqwest`;
- database connection pools, rows, transactions, cursors, or driver-specific errors;
- any other third-party implementation type that a plugin may reasonably need to use at a different version.

For example, this is **not allowed**:

```rust
pub trait DatabaseDriver {
    fn pool(&self) -> sqlx::Pool<sqlx::Postgres>;
}
```

Nor should a public type contain an implementation object:

```rust
pub struct ConnectionHandle {
    pub pool: sqlx::Pool<sqlx::Postgres>,
}
```

Instead, the API should expose an opaque, transport-neutral handle:

```rust
pub struct ConnectionHandle {
    pub id: String,
    pub pool_id: String,
}
```

The driver owns and manages the actual connection pool internally.

## Recommended dependency layering

The intended dependency graph is:

```text
                         DataZen Host
                              │
                    DataZen Driver API
                              │
              ┌───────────────┼───────────────┐
              │               │               │
          Driver A         Driver B        Driver C
              │               │               │
           sqlx 0.7         sqlx 0.8        mongodb
              │               │               │
           private          private         private
```

Multiple drivers may therefore depend on different versions of `sqlx` or use completely different database libraries. Cargo can resolve those dependencies independently as long as implementation types do not cross the Driver API boundary.

## Allowed foundation dependencies

### `serde` / `serde_json`

These are appropriate for transport-neutral data exchange, serialization, command input/output, and JSON Schema metadata. `serde_json::Value` is intentionally part of the API where generic JSON data is required.

### `async-trait`

`async-trait` is currently used to express asynchronous Driver traits. It is an API implementation dependency rather than a database implementation dependency. It may be re-exported for plugin ergonomics, but changes to it should be treated as part of the Driver API compatibility surface.

### `inventory`

`inventory` is used for compile-time Driver registration. It is intentionally part of the plugin integration mechanism because DataZen embeds Drivers into the application binary rather than dynamically loading Rust shared libraries.

## Opaque handles and transport-neutral results

Resources owned by a Driver should cross the API boundary through opaque identifiers or API-defined handles.

Good examples:

```rust
ConnectionHandle
QueryResult
MultiQueryResult
Value
DriverError
```

Bad examples:

```rust
sqlx::Pool
sqlx::Row
mongodb::Client
redis::aio::Connection
sqlx::Transaction
```

A Driver may use any of these implementation objects internally, but the Host must never need to know their concrete type.

## Versioning policy

A dependency should be considered part of the Driver API compatibility surface when its concrete types appear in a public API signature or public field.

Adding an implementation dependency internally does **not** require a Driver API protocol change. Exposing that dependency's concrete type **does** create a compatibility requirement and should normally be avoided.

Breaking changes to DataZen Driver traits or wire-level API types must follow the existing Driver API protocol-version policy.

## Review checklist

When changing `datazen-driver-api`, verify:

- [ ] No `sqlx` type appears in a public API.
- [ ] No database-specific crate type appears in a public API.
- [ ] No `tokio` type appears in a public API.
- [ ] Connection pools remain owned by the Driver.
- [ ] Rows, cursors, and transactions are represented by API-defined types or opaque handles.
- [ ] Errors crossing the boundary are represented by `DriverError` or another API-defined error type.
- [ ] Generic JSON data uses `serde_json` rather than a database-specific document type.
- [ ] New third-party dependencies are checked for accidental public exposure.
- [ ] Protocol-version implications are considered for changes to public traits and transport types.

## Design goal

The goal is not to prevent Drivers from using powerful database libraries. The goal is to keep those libraries **private to each Driver implementation**.

A Driver should be free to choose its own database client, runtime helpers, connection-pool implementation, and library versions, while DataZen interacts only with the stable `datazen-driver-api` contract.
