# DataZen Driver API

This document describes the public API that an external Datazen database driver plugin implements. It is intended for developers building a driver in an independent repository.

The API crate is the contract between a driver plugin and the Datazen host. A plugin is compiled into the host application at build time; it is not loaded through a Rust dynamic-library ABI.

## 1. API layers

The public API is organized around three main concepts:

```text
DatabaseDriverFactory
        │
        ├── driver_id()
        ├── create()
        ├── capabilities
        └── create_kv()
                │
                ▼
        DatabaseDriver
                │
                ├── connection lifecycle
                ├── metadata
                ├── query / execute
                ├── transactions
                ├── explain / cancellation
                ├── SQL dialect behavior
                ├── backup / restore
                └── structure editing

        KeyValueDriver (optional)
                │
                ├── scan keys
                └── inspect key
```

A typical SQL driver implements `DatabaseDriver` and `DatabaseDriverFactory`. A key-value driver can implement `KeyValueDriver`; the factory exposes it through `create_kv()`.

## 2. Minimal SQL driver

The smallest SQL driver must implement the following methods of `DatabaseDriver`:

```rust
use datazen_driver_api::{
    async_trait, register_driver, ConnectionConfig, ConnectionHandle,
    DatabaseDriver, DatabaseDriverFactory, DatabaseType, DriverError,
    QueryResult, MultiQueryResult, ServerInfo, TableInfo, TableSchema, Value,
};
use std::sync::Arc;

struct MyDriver;
struct MyDriverFactory;

#[async_trait]
impl DatabaseDriver for MyDriver {
    fn driver_type(&self) -> DatabaseType {
        "mydb".to_string()
    }

    async fn connect(
        &self,
        config: &ConnectionConfig,
    ) -> Result<ConnectionHandle, DriverError> {
        todo!()
    }

    async fn test_connection(
        &self,
        config: &ConnectionConfig,
    ) -> Result<ServerInfo, DriverError> {
        todo!()
    }

    async fn disconnect(
        &self,
        handle: ConnectionHandle,
    ) -> Result<(), DriverError> {
        todo!()
    }

    async fn get_databases(
        &self,
        handle: &ConnectionHandle,
    ) -> Result<Vec<String>, DriverError> {
        todo!()
    }

    async fn get_tables(
        &self,
        handle: &ConnectionHandle,
        database: &str,
    ) -> Result<Vec<TableInfo>, DriverError> {
        todo!()
    }

    async fn get_table_schema(
        &self,
        handle: &ConnectionHandle,
        table: &str,
    ) -> Result<TableSchema, DriverError> {
        todo!()
    }

    async fn query(
        &self,
        handle: &ConnectionHandle,
        sql: &str,
    ) -> Result<QueryResult, DriverError> {
        todo!()
    }

    async fn query_multi(
        &self,
        handle: &ConnectionHandle,
        sql: &str,
        limit: Option<u32>,
    ) -> Result<MultiQueryResult, DriverError> {
        todo!()
    }

    async fn query_with_params(
        &self,
        handle: &ConnectionHandle,
        sql: &str,
        params: &[Value],
    ) -> Result<QueryResult, DriverError> {
        todo!()
    }

    async fn execute(
        &self,
        handle: &ConnectionHandle,
        sql: &str,
    ) -> Result<u64, DriverError> {
        todo!()
    }

    async fn cancel_query(
        &self,
        handle: &ConnectionHandle,
    ) -> Result<(), DriverError> {
        todo!()
    }
}

impl DatabaseDriverFactory for MyDriverFactory {
    fn create(&self) -> Arc<dyn DatabaseDriver> {
        Arc::new(MyDriver)
    }

    fn driver_id(&self) -> &'static str {
        "mydb"
    }
}

register_driver!(&MyDriverFactory);
```

The required methods are the methods without a default implementation in the `DatabaseDriver` trait. Everything else is optional unless the feature you expose requires it.

## 3. `DatabaseDriverFactory`

`DatabaseDriverFactory` is the registration and construction interface. It is not the database implementation itself.

### `create()` — required

```rust
fn create(&self) -> Arc<dyn DatabaseDriver>;
```

Creates the driver implementation that Datazen will use for database operations.

The returned object must implement `DatabaseDriver` and be safe to share between threads (`Send + Sync`).

### `driver_id()` — required

```rust
fn driver_id(&self) -> &'static str;
```

Returns the stable identifier used to identify the plugin in the host.

This ID should be unique across all installed drivers. It is also the identifier used by the Datazen driver registry/build configuration.

Do not change it casually after the driver is published.

### `protocol_version()` — optional

Defaults to the current `PROTOCOL_VERSION`.

Override this only when a plugin needs to explicitly declare the API protocol version it was compiled against.

The API currently exposes:

```rust
pub const PROTOCOL_VERSION: u32 = 2;
pub const MIN_PROTOCOL_VERSION: u32 = 1;
```

Breaking changes to the core driver traits require a protocol-version change.

### Capability methods — optional

The factory exposes these host-level capability declarations:

| Method | Default | Purpose |
|---|---:|---|
| `supports_cancel_query()` | `false` | Whether query cancellation is supported |
| `supports_explain()` | `false` | Whether EXPLAIN is supported |
| `supports_streaming_results()` | `false` | Whether results can be streamed |
| `create_kv()` | `None` | Expose an optional `KeyValueDriver` implementation |

These values describe capabilities that the host may need before invoking the corresponding feature.

## 4. `register_driver!`

Every plugin must register its factory:

```rust
register_driver!(&MyDriverFactory);
```

The macro uses `inventory` to register the factory at link time. Datazen then iterates over all factories compiled into the application.

This is why a driver must be included in the Datazen build. The API is not a runtime `.so`/`.dylib` plugin ABI.

## 5. `DatabaseDriver`

`DatabaseDriver` is the main interface for SQL/database drivers.

It has three categories of methods:

1. **Required core methods** — must be implemented.
2. **Optional capability/dialect methods** — have sensible defaults and should only be overridden when the database differs from the defaults.
3. **Optional feature methods** — default to `NotSupported` or are implemented using other core APIs.

### 5.1 Required: `driver_type()`

```rust
fn driver_type(&self) -> DatabaseType;
```

Returns the database type handled by this implementation.

`DatabaseType` is a `String`, so external plugins can define their own identifiers without modifying the API crate.

It normally matches the driver's registry/UI database type.

### 5.2 Required: `connect()`

```rust
async fn connect(
    &self,
    config: &ConnectionConfig,
) -> Result<ConnectionHandle, DriverError>;
```

Creates the live connection/pool represented by `ConnectionHandle`.

The `ConnectionConfig` contains common connection information:

- host
- port
- database
- schema
- username/password
- SSL mode
- connection timeout
- SSH tunnel configuration
- opaque driver-specific `options`

A plugin should put database-specific connection settings in `ConnectionConfig::options` rather than requiring a change to the common API.

### 5.3 Required: `test_connection()`

```rust
async fn test_connection(
    &self,
    config: &ConnectionConfig,
) -> Result<ServerInfo, DriverError>;
```

Tests a connection configuration and returns server information.

Unlike `get_server_info()`, this method receives a configuration and is expected to establish whatever temporary connection/resources are required for the test.

### 5.4 Required: `disconnect()`

```rust
async fn disconnect(
    &self,
    handle: ConnectionHandle,
) -> Result<(), DriverError>;
```

Releases the connection/pool represented by the handle.

### 5.5 Required: `get_databases()`

```rust
async fn get_databases(
    &self,
    handle: &ConnectionHandle,
) -> Result<Vec<String>, DriverError>;
```

Returns databases/catalogs that Datazen can present in its database tree.

For database engines without a database concept, return the appropriate logical scope used by the engine.

### 5.6 Required: `get_tables()`

```rust
async fn get_tables(
    &self,
    handle: &ConnectionHandle,
    database: &str,
) -> Result<Vec<TableInfo>, DriverError>;
```

Lists tables/relations for the requested database scope.

`TableInfo` includes:

- name
- schema
- table type
- optional row count

### 5.7 Required: `get_table_schema()`

```rust
async fn get_table_schema(
    &self,
    handle: &ConnectionHandle,
    table: &str,
) -> Result<TableSchema, DriverError>;
```

Returns the structural metadata required by Datazen for a table.

`TableSchema` contains:

- columns
- primary keys
- indexes
- foreign keys

Each `ColumnSchema` includes the database type, nullability, default value, comment, primary-key flag, and auto-increment flag.

### 5.8 Required: `query()`

```rust
async fn query(
    &self,
    handle: &ConnectionHandle,
    sql: &str,
) -> Result<QueryResult, DriverError>;
```

Executes a single SQL query and returns its columns, rows, affected-row information, and execution time.

### 5.9 Required: `query_multi()`

```rust
async fn query_multi(
    &self,
    handle: &ConnectionHandle,
    sql: &str,
    limit: Option<u32>,
) -> Result<MultiQueryResult, DriverError>;
```

Executes SQL containing multiple statements and returns one `StatementResult` per statement.

`limit` can be used to bound result rows where supported by the driver's execution strategy.

### 5.10 Required: `query_with_params()`

```rust
async fn query_with_params(
    &self,
    handle: &ConnectionHandle,
    sql: &str,
    params: &[Value],
) -> Result<QueryResult, DriverError>;
```

Executes a parameterized query. The implementation is responsible for mapping `Value` values to the database client's native parameter representation.

Do not emulate parameter binding by concatenating untrusted values into SQL.

### 5.11 Required: `execute()`

```rust
async fn execute(
    &self,
    handle: &ConnectionHandle,
    sql: &str,
) -> Result<u64, DriverError>;
```

Executes a statement that does not primarily return a result set and returns the affected-row count when available.

### 5.12 Required: `cancel_query()`

```rust
async fn cancel_query(
    &self,
    handle: &ConnectionHandle,
) -> Result<(), DriverError>;
```

Cancels the active query associated with the connection/session.

The method itself is required by the trait. The factory's `supports_cancel_query()` tells the host whether the driver actually provides cancellation semantics.

If the underlying database cannot cancel queries, return an appropriate `DriverError` rather than pretending cancellation succeeded.

## 6. SQL dialect and behavior hooks

These methods already have defaults. Most drivers do not need to implement them.

### `driver_category()`

Defaults to `DriverCategory::Sql`.

Override it when the driver is not a normal SQL database, such as a key-value or document-oriented driver represented through the generic database interface.

### `quote_char()` and `quote_ident()`

Define identifier quoting behavior.

The default quote character is `"`. Override `quote_char()` for engines using a different identifier delimiter, such as backticks.

`quote_ident()` already escapes the configured quote character, so a driver normally only needs to override `quote_char()`.

### `skip_count_query()`

Defaults to `false`.

Set to `true` when Datazen should avoid issuing an additional count query for pagination because the database/engine does not support or benefit from that behavior.

### `supports_offset()`

Defaults to `true`.

Set to `false` when the SQL dialect does not support `OFFSET` pagination.

### `supports_explain()`

Defaults to `true` at the driver level.

Set it according to the actual database capability. The factory also exposes a capability flag used by the host.

### `format_sql_literal()`

Converts a `Value` into a SQL literal. The default handles null, booleans, integers, floats, strings, bytes, timestamps, and JSON.

Override it when the target database has different literal syntax.

### `build_update_sql()`

Builds the standard single-table `UPDATE` statement used for row editing. Override it when the database has non-standard update syntax or requires special handling.

## 7. Optional query and session features

### Transactions

```rust
async fn begin_transaction(...)
async fn commit(...)
async fn rollback(...)
```

These default to `DriverError::TransactionError("Not supported ...")`.

Implement them when the database supports transactions and Datazen should expose transactional operations.

### `explain()`

Returns an `ExplainResult` containing plan text and optional structured plan/cost information.

The default implementation returns an unsupported error. Implement it when the database provides query-plan analysis.

### `get_server_info()`

Returns server information using an **existing** `ConnectionHandle`.

This is intentionally different from `test_connection()`: use `get_server_info()` when Datazen already has a live connection and needs server version/type information without creating another temporary connection.

### `use_database()`

Switches the active database/session for subsequent queries.

The default is a no-op because many drivers select the database when connecting. Override it for engines where database selection is a session operation.

## 8. SQL command API

### `command_definitions()`

Returns commands supported by the driver.

The default SQL driver exposes the standard `query` and `execute` commands. A plugin with driver-specific operations can override this method and append additional command definitions.

### `execute_command()`

Executes a named driver command.

The default implementation dispatches `query` and `execute` to the standard SQL APIs. Override it when the driver has additional commands that do not fit the generic SQL interface.

This is the preferred extension point for driver-specific operations rather than adding application-specific dispatch paths.

## 9. Backup and restore

### `dump_table_ddl()`

Produces `CREATE TABLE` DDL for a table. The default implementation derives the DDL from `get_table_schema()`.

Override it when the database's DDL syntax cannot be represented by the generic schema-to-DDL implementation.

### `dump_database()`

Produces a SQL dump for a database.

The default implementation uses the generic SQL dump implementation. `BackupDumpOptions` controls schema/data-only output, clean drops, database creation, owner handling, transaction hints, routines, and triggers.

If the driver needs database-native dumping behavior, override this method.

### `restore_sql()`

Restores SQL by splitting the dump into statements and executing them through the driver. The default implementation supports the generic restore flow and transaction option.

Override it when the database requires a specialized restore mechanism or statement parser.

## 10. Structure editing API

### `structure_capabilities()`

Reports which table/column/index editing operations the database supports.

The default implementation disables all structure editing capabilities and sets `dialect_id` to the driver's type.

Capabilities include:

- create/drop/rename columns
- change type/nullability/default
- primary-key changes
- column reordering
- comments
- create/drop/rebuild indexes
- index type/include/filter/comment
- supported index methods
- alteration strategy

A driver should only advertise operations that it can safely implement.

### `plan_structure_changes()`

Converts a `StructureChangeRequest` into a `StructureChangePlan` containing the database-specific operations needed to apply the requested table changes.

The default implementation returns `DriverError::Unsupported`.

Implement this together with `structure_capabilities()` when the driver supports Datazen's table structure editor.

## 11. AI prompt customization

### `prompt_overrides()`

Returns driver-specific prompt templates for AI features.

The default returns an empty map, meaning Datazen's global prompts are used.

A plugin can override individual `PromptScenario` values when the database has terminology, SQL syntax, metadata conventions, or query-planning behavior that requires specialized instructions.

Templates use `{{variable}}` placeholders. The available variables depend on the scenario and are defined by the Datazen host.

## 12. `KeyValueDriver`

Key-value databases can additionally implement:

```rust
#[async_trait]
pub trait KeyValueDriver: Send + Sync {
    fn driver_type(&self) -> DatabaseType;

    async fn scan_keys_with_info(
        &self,
        handle: &ConnectionHandle,
        db_index: u32,
        pattern: &str,
        cursor: u64,
        count: u32,
    ) -> Result<(u64, Vec<KeyEntry>, u64), DriverError>;

    async fn get_key_detail(
        &self,
        handle: &ConnectionHandle,
        db_index: u32,
        key: &str,
    ) -> Result<KeyDetail, DriverError>;
}
```

All three methods are required if `KeyValueDriver` is implemented.

### `driver_type()`

Returns the key-value database type handled by the implementation.

### `scan_keys_with_info()`

Scans keys using cursor-based pagination and returns:

1. the next cursor,
2. key metadata entries,
3. the total count when available.

`pattern` is the key matching pattern and `count` is the requested scan batch size.

`KeyEntry` contains the key name, key type, TTL, size, and a preview.

### `get_key_detail()`

Returns the detailed value and metadata for a specific key.

`KeyDetail` contains the key name, type, TTL, and JSON representation of the value.

If the driver supports both normal database operations and key-value operations, return the `KeyValueDriver` implementation from the factory's `create_kv()` method.

## 13. Shared data types

The API crate provides the data structures used across the trait boundary.

### `ConnectionConfig`

Common connection configuration plus an opaque `options` map for driver-specific settings.

### `ConnectionHandle`

Opaque connection identity held by Datazen. The plugin decides how its `id` and `pool_id` map to its internal connection/pool state.

### `Value`

The generic database value representation:

```text
Null
Bool
Integer
Float
String
Bytes
Timestamp
Json
```

Drivers are responsible for converting their native database values to/from this representation.

### Query results

`QueryResult` represents one result set. `MultiQueryResult` contains multiple `StatementResult` values.

### Metadata

The main schema types are:

```text
TableInfo
TableSchema
ColumnSchema
IndexInfo
ForeignKeyInfo
```

Keep these structures accurate because Datazen uses them for navigation, data editing, schema display, and other higher-level features.

### `DriverError`

Use the most specific available error variant:

- `ConnectionFailed`
- `QueryFailed`
- `ConnectionTimeout`
- `AuthenticationFailed`
- `SslError`
- `SshTunnelError`
- `InvalidConfig`
- `DriverNotFound`
- `PoolExhausted`
- `TransactionError`
- `NotSupported`
- `Unsupported`

Do not hide an authentication, TLS, timeout, or configuration failure as a generic query error when a more specific variant applies.

## 14. What must a plugin implement?

### SQL driver

At minimum:

**`DatabaseDriver`**

- `driver_type`
- `connect`
- `test_connection`
- `disconnect`
- `get_databases`
- `get_tables`
- `get_table_schema`
- `query`
- `query_multi`
- `query_with_params`
- `execute`
- `cancel_query`

**`DatabaseDriverFactory`**

- `create`
- `driver_id`

**Registration**

- `register_driver!(&YourFactory)`

Everything else has a default implementation or is an optional capability.

### Key-value driver

If the plugin implements `KeyValueDriver`, all of its methods are required:

- `driver_type`
- `scan_keys_with_info`
- `get_key_detail`

Expose it from the factory using `create_kv()`.

## 15. Recommended implementation order

For a new SQL database, implement in this order:

1. `DatabaseDriverFactory`
2. `driver_type`
3. `connect` / `disconnect`
4. `test_connection`
5. `get_databases`
6. `get_tables`
7. `get_table_schema`
8. `query`
9. `query_multi`
10. `query_with_params`
11. `execute`
12. `cancel_query`
13. Dialect overrides such as `quote_char`, `supports_offset`, and `supports_explain`
14. Transactions and `explain` if supported
15. Backup/restore overrides if the generic implementation is insufficient
16. Structure editing capabilities if supported
17. Driver-specific commands
18. AI prompt overrides if useful

This order gets a usable driver running first and adds optional Datazen features incrementally.

## 16. Compatibility and protocol versioning

The API exposes:

```rust
pub const PROTOCOL_VERSION: u32 = 2;
pub const MIN_PROTOCOL_VERSION: u32 = 1;
```

A breaking change to the core traits requires a protocol version change. Plugin authors should pin a compatible API version in their plugin repository and validate the plugin against the Datazen version they intend to support.

The host can reject plugins below `MIN_PROTOCOL_VERSION`. Plugins between the minimum supported version and the current protocol may run with newer capabilities unavailable to them defaulting to disabled behavior.

## 17. Practical rule for plugin authors

Do not implement every method just because it exists.

Start with the required methods, make their behavior correct, and then advertise/implement optional capabilities only when the underlying database genuinely supports them.

In particular:

- Do not claim query cancellation if the database client cannot cancel it.
- Do not claim EXPLAIN support if there is no meaningful plan API.
- Do not enable structure editing without implementing safe dialect-specific planning.
- Do not concatenate parameter values into SQL in `query_with_params`.
- Use `ConnectionConfig::options` for plugin-specific connection settings.
- Preserve a stable `driver_id()` once the plugin is published.

The goal of the API is to keep the host independent of database-specific implementation details while giving each plugin enough extension points to expose the features its database actually supports.
