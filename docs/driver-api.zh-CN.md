# DataZen Driver API 中文指南

本文介绍 Datazen Driver API 的公开接口，面向在独立 Git 仓库中开发 Datazen 数据库插件的开发者。

Driver API 是 Plugin 与 Datazen Host 之间的契约。Plugin 会在 Datazen **编译期**被编译并链接进应用，而不是通过 Rust 动态库 ABI 在运行时加载。

## 1. API 分层

公共 API 可以理解为三个主要层次：

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
                ├── 连接生命周期
                ├── 元数据
                ├── Query / Execute
                ├── Transaction
                ├── Explain / Cancel
                ├── SQL Dialect
                ├── Backup / Restore
                └── Structure Editing

        KeyValueDriver（可选）
                │
                ├── Scan Keys
                └── Key Detail
```

普通 SQL Driver 通常实现 `DatabaseDriver` 和 `DatabaseDriverFactory`。Key-Value 类型 Driver 可以额外实现 `KeyValueDriver`，并由 Factory 的 `create_kv()` 暴露给 Host。

## 2. 最小 SQL Driver

一个最小 SQL Driver 至少需要实现 `DatabaseDriver` 中没有默认实现的方法，以及 Factory 的核心方法：

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

判断一个接口是否必须实现的最简单方法：查看 `DatabaseDriver` trait 中该方法是否有默认实现。没有默认实现的方法必须实现；有默认实现的方法只有在数据库真正支持或需要特殊行为时才需要 override。

## 3. `DatabaseDriverFactory`

`DatabaseDriverFactory` 是 Driver 的注册与创建接口，不是数据库操作本身。

### `create()` — 必须实现

```rust
fn create(&self) -> Arc<dyn DatabaseDriver>;
```

创建 Datazen 实际使用的 Driver 对象。

返回对象必须实现 `DatabaseDriver`，并满足线程安全要求（`Send + Sync`）。

### `driver_id()` — 必须实现

```rust
fn driver_id(&self) -> &'static str;
```

返回 Driver 的稳定唯一标识。

这个 ID 会用于 Datazen 的 Driver registry/build configuration，也应该与插件的前端 database type 保持一致。

插件发布以后不要随意修改这个 ID。

### `protocol_version()` — 可选

默认使用当前 `PROTOCOL_VERSION`。

只有在插件需要明确声明自身编译时所依赖的 API protocol version 时才需要 override。

当前 API：

```rust
pub const PROTOCOL_VERSION: u32 = 2;
pub const MIN_PROTOCOL_VERSION: u32 = 1;
```

核心 Driver trait 发生 breaking change 时，需要提升 protocol version。

### Factory Capability 方法 — 可选

| 方法 | 默认值 | 作用 |
|---|---:|---|
| `supports_cancel_query()` | `false` | 是否支持 Query Cancel |
| `supports_explain()` | `false` | 是否支持 EXPLAIN |
| `supports_streaming_results()` | `false` | 是否支持流式结果 |
| `create_kv()` | `None` | 暴露可选的 `KeyValueDriver` |

这些声明用于告诉 Host 某项能力是否可以安全调用。

## 4. `register_driver!`

每个 Plugin 都必须注册自己的 Factory：

```rust
register_driver!(&MyDriverFactory);
```

该宏通过 `inventory` 在链接期注册 Factory。Datazen 启动后会遍历所有已经编译进 binary 的 Driver Factory。

因此 Driver 必须被包含在 Datazen 的编译结果中。这里不是 runtime `.so` / `.dylib` Plugin ABI。

## 5. `DatabaseDriver`

`DatabaseDriver` 是 SQL/数据库 Driver 的主要接口。

可以把它分成三类：

1. **核心必选接口**：必须实现。
2. **可选能力和 Dialect 接口**：有默认实现，需要时 override。
3. **可选功能接口**：通常默认返回 `NotSupported`，或者通过其他核心 API 提供通用实现。

### 5.1 `driver_type()` — 必须实现

```rust
fn driver_type(&self) -> DatabaseType;
```

返回当前 Driver 所处理的数据库类型。

`DatabaseType` 本质上是 `String`，因此外部 Plugin 可以定义自己的 database type，不需要修改 API crate。

通常应该与 Factory 的 `driver_id` 以及 Datazen 前端注册的 database type 保持一致。

### 5.2 `connect()` — 必须实现

```rust
async fn connect(
    &self,
    config: &ConnectionConfig,
) -> Result<ConnectionHandle, DriverError>;
```

根据连接配置创建真实连接/连接池，并返回 `ConnectionHandle`。

`ConnectionConfig` 包含通用连接信息：

- host
- port
- database
- schema
- username/password
- SSL mode
- connection timeout
- SSH tunnel configuration
- Driver-specific `options`

数据库特有的配置应该尽量放到 `ConnectionConfig::options` 中，而不是修改公共 API。

### 5.3 `test_connection()` — 必须实现

```rust
async fn test_connection(
    &self,
    config: &ConnectionConfig,
) -> Result<ServerInfo, DriverError>;
```

测试指定连接配置是否可用，并返回服务器信息。

与 `get_server_info()` 的区别是：`test_connection()` 接收配置，通常需要建立临时连接；`get_server_info()` 使用已经存在的 `ConnectionHandle`。

### 5.4 `disconnect()` — 必须实现

```rust
async fn disconnect(
    &self,
    handle: ConnectionHandle,
) -> Result<(), DriverError>;
```

释放 `ConnectionHandle` 对应的连接/连接池资源。

### 5.5 `get_databases()` — 必须实现

```rust
async fn get_databases(
    &self,
    handle: &ConnectionHandle,
) -> Result<Vec<String>, DriverError>;
```

返回 Datazen Database Tree 中可以展示的 database/catalog 列表。

如果数据库没有传统意义上的 database 概念，应返回该数据库适合用于树形导航的逻辑 scope。

### 5.6 `get_tables()` — 必须实现

```rust
async fn get_tables(
    &self,
    handle: &ConnectionHandle,
    database: &str,
) -> Result<Vec<TableInfo>, DriverError>;
```

返回指定 database scope 下的表/关系列表。

`TableInfo` 包含：

- name
- schema
- table type
- 可选 row count

### 5.7 `get_table_schema()` — 必须实现

```rust
async fn get_table_schema(
    &self,
    handle: &ConnectionHandle,
    table: &str,
) -> Result<TableSchema, DriverError>;
```

返回 Datazen 操作表所需的结构元数据。

`TableSchema` 主要包含：

- columns
- primary keys
- indexes
- foreign keys

`ColumnSchema` 包含数据库类型、是否允许 NULL、默认值、comment、primary-key 标志以及 auto-increment 标志等信息。

### 5.8 `query()` — 必须实现

```rust
async fn query(
    &self,
    handle: &ConnectionHandle,
    sql: &str,
) -> Result<QueryResult, DriverError>;
```

执行一个 SQL Query，并返回 columns、rows、affected rows 和 execution time 等信息。

### 5.9 `query_multi()` — 必须实现

```rust
async fn query_multi(
    &self,
    handle: &ConnectionHandle,
    sql: &str,
    limit: Option<u32>,
) -> Result<MultiQueryResult, DriverError>;
```

执行包含多个 statement 的 SQL，并返回每个 statement 对应的结果。

### 5.10 `query_with_params()` — 必须实现

```rust
async fn query_with_params(
    &self,
    handle: &ConnectionHandle,
    sql: &str,
    params: &[Value],
) -> Result<QueryResult, DriverError>;
```

执行参数化 Query。Driver 负责将 API 的 `Value` 转换为底层数据库客户端的参数类型。

**不要通过字符串拼接参数生成 SQL。** 应使用底层数据库客户端真正的 parameter binding 能力。

### 5.11 `execute()` — 必须实现

```rust
async fn execute(
    &self,
    handle: &ConnectionHandle,
    sql: &str,
) -> Result<u64, DriverError>;
```

执行主要用于写操作或其他不返回 result set 的 statement，并返回 affected row count（如果数据库支持）。

### 5.12 `cancel_query()` — 必须实现

```rust
async fn cancel_query(
    &self,
    handle: &ConnectionHandle,
) -> Result<(), DriverError>;
```

取消当前连接/session 上正在执行的 Query。

虽然 trait 方法本身需要实现，但 Factory 的 `supports_cancel_query()` 用于告诉 Host 该 Driver 是否真正支持 Query Cancel。

如果底层数据库不支持取消，不应该伪装成成功，应返回适当的 `DriverError`。

## 6. SQL Dialect 和行为扩展

以下接口有默认实现，大多数 Driver 不需要实现。

### `driver_category()`

默认是 `DriverCategory::Sql`。

对于不是传统 SQL 数据库的 Driver，可以 override。

### `quote_char()` / `quote_ident()`

定义数据库的 identifier quoting 规则。

默认 quote character 是 `"`。如果数据库使用其他 delimiter，例如反引号，应 override `quote_char()`。

`quote_ident()` 已经会处理 quote character 的转义，因此大部分 Driver 只需要修改 `quote_char()`。

### `skip_count_query()`

默认 `false`。

如果数据库不适合额外执行 count query，可以设置为 `true`。

### `supports_offset()`

默认 `true`。

如果 SQL dialect 不支持 `OFFSET`，设置为 `false`。

### `supports_explain()`

Driver 层默认允许 EXPLAIN，但实际应该根据数据库能力决定。Factory 也提供相应的 capability flag 供 Host 使用。

### `format_sql_literal()`

把 API `Value` 转换为 SQL literal。默认处理 NULL、boolean、整数、浮点数、字符串、bytes、timestamp 和 JSON。

如果目标数据库的 literal syntax 不同，需要 override。

### `build_update_sql()`

生成 Datazen 行编辑使用的标准单表 UPDATE SQL。

如果数据库 UPDATE syntax 特殊，可以 override。

## 7. Query / Session 可选功能

### Transaction

```rust
async fn begin_transaction(...)
async fn commit(...)
async fn rollback(...)
```

默认返回不支持 Transaction 的错误。

如果数据库支持事务并且 Datazen 应该提供事务能力，则实现这些方法。

### `explain()`

返回 `ExplainResult`，包含 plan text，以及可选的结构化 plan/cost 信息。

默认返回 unsupported error。数据库支持执行计划分析时再实现。

### `get_server_info()`

使用已经存在的 `ConnectionHandle` 获取服务器信息。

与 `test_connection()` 不同，它不应该为了获取信息而重复建立临时连接。

### `use_database()`

切换当前 session 后续 Query 使用的 database。

默认实现为空操作，因为很多数据库在建立连接时已经确定 database。如果数据库需要通过 session command 切换 database，则 override。

## 8. SQL Command API

### `command_definitions()`

返回 Driver 支持的 command 定义。

默认提供标准的 `query` 和 `execute` command。

如果数据库存在无法通过通用 SQL interface 表达的 Driver-specific operation，可以 override 并添加 command。

### `execute_command()`

执行指定名称的 Driver command。

默认实现会将 `query` 和 `execute` 转发到标准 SQL API。Driver 有额外 command 时，可以 override。

这是实现 Driver-specific operation 的推荐扩展点，而不是修改 Datazen 应用层的 dispatch logic。

## 9. Backup / Restore

### `dump_table_ddl()`

生成指定 table 的 `CREATE TABLE` DDL。

默认实现可以根据 `get_table_schema()` 生成通用 DDL。如果数据库语法差异较大，应 override。

### `dump_database()`

生成数据库 SQL dump。

`BackupDumpOptions` 可以控制 schema-only、data-only、clean drops、database creation、owner、transaction hints、routines 和 triggers 等行为。

如果数据库有自己的 dump API，应 override 为数据库原生实现。

### `restore_sql()`

恢复 SQL dump。默认实现将 dump 分割成 statement，然后通过 Driver 执行，并支持通用 transaction 选项。

如果数据库需要特殊 restore 机制或 statement parser，则 override。

## 10. Structure Editing API

### `structure_capabilities()`

告诉 Datazen 当前数据库支持哪些表结构修改操作。

默认关闭所有 structure editing capability，并将 `dialect_id` 设置为 Driver type。

支持的能力包括：

- create/drop/rename column
- 修改 type/nullability/default
- primary key 修改
- column reorder
- comment
- create/drop/rebuild index
- index type/include/filter/comment
- index methods
- alteration strategy

**只应该声明底层数据库真正支持且实现安全的能力。**

### `plan_structure_changes()`

将 `StructureChangeRequest` 转换为数据库特定的 `StructureChangePlan`，其中包含实际需要执行的结构修改操作。

默认返回 `DriverError::Unsupported`。

如果 Driver 支持 Datazen 的表结构编辑器，应同时实现 `structure_capabilities()` 和 `plan_structure_changes()`。

## 11. AI Prompt 定制

### `prompt_overrides()`

提供数据库特定的 AI Prompt 模板。

默认返回空 map，使用 Datazen 全局 Prompt。

如果数据库有特殊 SQL syntax、metadata、术语或 Query Planning 方式，可以 override 特定 `PromptScenario`。

模板使用 `{{variable}}` 占位符，可用变量取决于 scenario 和 Datazen Host 定义。

## 12. `KeyValueDriver`

Key-Value 类型数据库可以额外实现：

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

如果实现 `KeyValueDriver`，上面三个接口都必须实现。

### `driver_type()`

返回 Key-Value Driver 对应的数据库类型。

### `scan_keys_with_info()`

使用 cursor 分页扫描 key，并返回：

1. next cursor
2. key metadata
3. total count（如果可以获取）

`pattern` 是 key matching pattern，`count` 是期望的扫描批次大小。

`KeyEntry` 包含 key name、key type、TTL、size 和 preview。

### `get_key_detail()`

获取指定 key 的详细内容和 metadata。

`KeyDetail` 包含 key name、type、TTL 以及 value 的 JSON 表示。

如果 Driver 同时支持普通数据库 API 和 Key-Value API，则通过 Factory 的 `create_kv()` 返回 `KeyValueDriver`。

## 13. 公共数据类型

### `ConnectionConfig`

包含通用连接参数，以及用于 Driver-specific settings 的 `options` map。

### `ConnectionHandle`

Datazen 持有的不透明连接标识。Plugin 自己决定 `id` 和 `pool_id` 如何映射到内部 connection/pool。

### `Value`

通用数据库 Value 类型：

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

Driver 负责在数据库原生类型和这个通用类型之间进行转换。

### Query Results

`QueryResult` 表示一个 result set；`MultiQueryResult` 包含多个 `StatementResult`。

### Metadata

主要 schema 类型：

```text
TableInfo
TableSchema
ColumnSchema
IndexInfo
ForeignKeyInfo
```

这些信息会被 Datazen 用于数据库树、数据编辑、Schema 展示等功能，因此应尽量准确。

### `DriverError`

应该尽量使用最具体的错误类型：

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

不要把认证失败、TLS 错误、timeout 或配置错误全部包装成普通 Query Error。

## 14. 一个 Plugin 到底必须实现什么？

### SQL Driver

**`DatabaseDriver` 必须实现：**

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

**`DatabaseDriverFactory` 必须实现：**

- `create`
- `driver_id`

**必须注册：**

```rust
register_driver!(&YourFactory);
```

除此之外的接口都有默认实现，或者属于可选能力。

### Key-Value Driver

如果实现 `KeyValueDriver`，必须实现：

- `driver_type`
- `scan_keys_with_info`
- `get_key_detail`

并通过 Factory 的 `create_kv()` 暴露给 Host。

## 15. 推荐实现顺序

开发新的 SQL Driver 时建议按照以下顺序：

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
13. Dialect overrides，例如 `quote_char`、`supports_offset`、`supports_explain`
14. Transaction 和 `explain`
15. Backup / Restore 特殊实现
16. Structure Editor capability
17. Driver-specific commands
18. AI prompt overrides

这样可以先快速实现一个可用 Driver，再逐步增加 Datazen 的高级功能。

## 16. Compatibility / Protocol Version

API 当前提供：

```rust
pub const PROTOCOL_VERSION: u32 = 2;
pub const MIN_PROTOCOL_VERSION: u32 = 1;
```

核心 trait 发生 breaking change 时必须修改 protocol version。Plugin 开发者应该在自己的 Plugin repository 中锁定兼容的 API 版本，并针对目标 Datazen 版本进行验证。

Host 可以拒绝低于 `MIN_PROTOCOL_VERSION` 的 Plugin。对于最低版本和当前版本之间的 Plugin，新版本增加但 Plugin 没有实现的能力应继续使用默认行为。

## 17. Plugin 开发原则

**不要因为 trait 中存在某个接口，就把所有接口都实现一遍。**

先实现必需接口，保证核心行为正确；然后只对数据库真实支持的能力进行 override 和 capability declaration。

尤其需要注意：

- 数据库客户端不能取消 Query，就不要声明支持 Query Cancel。
- 数据库没有真正的执行计划 API，就不要声明 EXPLAIN。
- 没有实现安全的数据库特定 planning，就不要开启 Structure Editing。
- `query_with_params()` 不要把参数拼接进 SQL。
- 数据库特有连接参数使用 `ConnectionConfig::options`。
- Plugin 发布以后保持稳定的 `driver_id()`。

Driver API 的目标是让 Datazen Host 不需要了解数据库内部实现，同时给每个 Plugin 足够的扩展点来暴露数据库真正支持的能力。
