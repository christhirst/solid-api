# `solid-api` — SolidFlow gRPC Management Service & MCP Server

`solid-api` is a pure **Tonic gRPC** microservice, CLI, and **Model Context Protocol (MCP)** server written in Rust (2024 edition) for provisioning and fully managing the **SolidFlow** application (`solid.grpcfront`) via SurrealDB.

It manages all SolidFlow resources directly in SurrealDB:
- **Users & Platform Roles** (`user_profile`): `admin`, `creator`, `user`, `viewer`, `roleOverride`, `simulatedRole`, personal `namespace`
- **Hierarchical Spaces / Groups** (`group_namespace` & `group_member`): dot-separated group hierarchy (e.g. `engineering.backend`) and granular rights (`view`, `use`, `write`, `delete`, `manager`)
- **SurrealDB Database Users**: native `DEFINE USER ... ON DATABASE PASSWORD ... ROLES ...`
- **Dashboards** (`dashboard`): interactive widget layouts (`buttons` JSON array), public slugs (`namespace/slug`), and space tags
- **Workflows** (`workflow`): multi-step automation pipelines (`steps` JSON array), visual DAG (`graph` JSON object), cron schedules, and workflow cloning
- **Connections** (`connection`): reusable connection profiles (`http`, `grpc`, `surrealdb`, `oauth`, `s3`)
- **Proto Registry & Trusted CAs** (`proto_file` & `ca_cert`): `.proto` schemas and PEM root certificates
- **Databases & SurrealQL** (`database_meta`): database creation, space tags, and arbitrary SurrealQL query execution

---

## Configuration (`config-rs`)

Configuration is loaded using [`config-rs`](https://docs.rs/config) from [`config/default.toml`](config/default.toml), optional `config/local.toml`, or a custom file path via `SOLID_API_CONFIG_PATH`.

### `config/default.toml`

```toml
[server]
listen_addr = "0.0.0.0:50051"

[surrealdb]
endpoint = "wss://app.ux-ti.com/rpc"
namespace = "solidflow"
database = "main"
username = "root"
password = "root"
```

### Environment Variable Overrides

| Environment Variable | Description | Default |
| :--- | :--- | :--- |
| `SOLID_API_CONFIG_PATH` | Path to custom `.toml` config file | `config/default.toml` |
| `GRPC_PORT` | Override gRPC port (`0.0.0.0:<GRPC_PORT>`) | `50051` |
| `SOLID_API__SERVER__LISTEN_ADDR` | Full gRPC bind address | `0.0.0.0:50051` |
| `SURREALDB_URL` / `SOLID_API__SURREALDB__ENDPOINT` | SurrealDB RPC endpoint (`ws://`, `wss://`, `http://`, `memory`) | `wss://app.ux-ti.com/rpc` |
| `SURREALDB_NS` / `SOLID_API__SURREALDB__NAMESPACE` | SurrealDB namespace | `solidflow` |
| `SURREALDB_DB` / `SOLID_API__SURREALDB__DATABASE` | SurrealDB database | `main` |
| `SURREALDB_USER` / `SOLID_API__SURREALDB__USERNAME` | SurrealDB username | `root` |
| `SURREALDB_PASS` / `SOLID_API__SURREALDB__PASSWORD` | SurrealDB password | `root` |
| `RUST_LOG` | Tracing filter directive | `info,solid_api=debug` |

---

## Running `solid-api`

### Start the Tonic gRPC Server

```bash
cargo run -- serve --listen-addr 0.0.0.0:50051
```

Or with an in-memory SurrealDB instance for local testing:

```bash
cargo run -- --endpoint memory serve --listen-addr 0.0.0.0:50051
```

---

## Model Context Protocol (MCP) Server for AI Agents

`solid-api` includes a built-in JSON-RPC 2.0 MCP server over `stdio` so AI assistants (Claude Desktop, Cursor, Antigravity, etc.) can inspect and manage all SolidFlow resources.

### MCP Client Configuration (`mcp_config.json`)

```json
{
  "mcpServers": {
    "solid-api": {
      "command": "/path/to/solid-api",
      "args": ["mcp"],
      "env": {
        "SURREALDB_URL": "wss://app.ux-ti.com/rpc",
        "SURREALDB_NS": "solidflow",
        "SURREALDB_DB": "main",
        "SURREALDB_USER": "root",
        "SURREALDB_PASS": "root"
      }
    }
  }
}
```

### Exposed MCP Tools (34 Tools)

- **Schema**: `init_schema`
- **Users & Roles**: `provision_user`, `get_user`, `list_users`, `assign_platform_role`, `delete_user`, `get_effective_access`
- **Groups / Spaces**: `create_group`, `list_groups`, `assign_group_member`, `remove_group_member`
- **SurrealDB DB Users**: `provision_db_user`, `list_db_users`
- **Dashboards**: `create_dashboard`, `get_dashboard`, `list_dashboards`, `update_dashboard`, `delete_dashboard`, `save_user_namespace`
- **Workflows**: `create_workflow`, `get_workflow`, `list_workflows`, `update_workflow`, `delete_workflow`, `clone_workflow`
- **Connections**: `create_connection`, `get_connection`, `list_connections`, `update_connection`, `delete_connection`
- **Proto Registry**: `upload_proto`, `get_proto`, `list_protos`, `delete_proto`
- **Trusted CAs**: `upload_ca_cert`, `get_ca_cert`, `list_ca_certs`, `delete_ca_cert`
- **Databases & SurrealQL**: `list_databases`, `create_database`, `execute_surreal_query`

---

## Complete gRPC API Reference (`proto/solid/v1/solid.proto`)

All gRPC services are defined in [`proto/solid/v1/solid.proto`](proto/solid/v1/solid.proto) under package `solid.v1`.
Below are [`grpcurl`](https://github.com/fullstorydev/grpcurl) examples for every service and RPC method.

---

### 1. `solid.v1.UserProvisioningService`

Manages `user_profile`, platform roles (`admin`, `creator`, `user`, `viewer`), hierarchical `group_namespace` / `group_member` permissions, and native SurrealDB database users.

#### `InitSchema`
Initializes all 10 SurrealDB tables and unique indexes.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{}' \
  localhost:50051 solid.v1.UserProvisioningService/InitSchema
```

#### `ProvisionUser`
Creates a new user profile in `user_profile:<sub>`.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "sub": "usr_alice_01",
    "username": "alice",
    "name": "Alice Admin",
    "email": "alice@example.com",
    "role": "admin",
    "namespace": "alice-ops"
  }' \
  localhost:50051 solid.v1.UserProvisioningService/ProvisionUser
```

#### `GetUser`
Retrieves a user profile by `sub`, `username`, or `email`.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"sub_or_username": "alice"}' \
  localhost:50051 solid.v1.UserProvisioningService/GetUser
```

#### `ListUsers`
Lists registered user profiles.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"limit": 50}' \
  localhost:50051 solid.v1.UserProvisioningService/ListUsers
```

#### `UpdateUser`
Updates a user's profile fields (`name`, `email`, `namespace`, `role`, `role_override`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "sub_or_username": "alice",
    "name": "Alice Principal Engineer",
    "namespace": "alice-cloud"
  }' \
  localhost:50051 solid.v1.UserProvisioningService/UpdateUser
```

#### `AssignPlatformRole`
Assigns or overrides a user's platform role (`admin`, `creator`, `user`, `viewer`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "sub_or_username": "alice",
    "role": "creator",
    "as_override": true
  }' \
  localhost:50051 solid.v1.UserProvisioningService/AssignPlatformRole
```

#### `GetEffectiveAccess`
Computes the user's effective platform role, boolean capability flags (`PlatformCapabilitiesMsg`), and hierarchical group grants.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"sub_or_username": "alice"}' \
  localhost:50051 solid.v1.UserProvisioningService/GetEffectiveAccess
```

#### `DeleteUser`
Deletes a user profile and removes their group memberships.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"sub_or_username": "alice"}' \
  localhost:50051 solid.v1.UserProvisioningService/DeleteUser
```

#### `CreateGroup`
Creates a root or hierarchical group namespace (e.g. `engineering` or `engineering.backend`) and grants the owner `manager` rights.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "name": "engineering.backend",
    "owner": "usr_alice_01",
    "description": "Backend Platform Team"
  }' \
  localhost:50051 solid.v1.UserProvisioningService/CreateGroup
```

#### `GetGroup`
Gets a group namespace by name.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"name": "engineering.backend"}' \
  localhost:50051 solid.v1.UserProvisioningService/GetGroup
```

#### `ListGroups`
Lists all group namespaces.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{}' \
  localhost:50051 solid.v1.UserProvisioningService/ListGroups
```

#### `DeleteGroup`
Deletes a group namespace and cleans up its members.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"name": "engineering.backend"}' \
  localhost:50051 solid.v1.UserProvisioningService/DeleteGroup
```

#### `AssignGroupMember`
Assigns a user to a group with granular rights (`view`, `use`, `write`, `delete`, `manager`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "group_name": "engineering.backend",
    "username": "bob",
    "rights": ["view", "use", "write"],
    "assigned_by": "usr_alice_01"
  }' \
  localhost:50051 solid.v1.UserProvisioningService/AssignGroupMember
```

#### `RemoveGroupMember`
Removes a user from a group.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "group_name": "engineering.backend",
    "username": "bob"
  }' \
  localhost:50051 solid.v1.UserProvisioningService/RemoveGroupMember
```

#### `ListGroupMembers`
Lists all members and their rights in a group.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"group_name": "engineering.backend"}' \
  localhost:50051 solid.v1.UserProvisioningService/ListGroupMembers
```

#### `ListUserGroups`
Lists all group memberships for a user.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"sub_or_username": "bob"}' \
  localhost:50051 solid.v1.UserProvisioningService/ListUserGroups
```

#### `ProvisionDbUser`
Defines a native SurrealDB database-level user (`DEFINE USER ... ON DATABASE PASSWORD ... ROLES ...`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "username": "svc_worker",
    "password": "StrongPassword123!",
    "roles": ["EDITOR"]
  }' \
  localhost:50051 solid.v1.UserProvisioningService/ProvisionDbUser
```

#### `ListDbUsers`
Lists SurrealDB database-level users (`INFO FOR DB`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{}' \
  localhost:50051 solid.v1.UserProvisioningService/ListDbUsers
```

#### `RemoveDbUser`
Removes a SurrealDB database-level user (`REMOVE USER ... ON DATABASE`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"username": "svc_worker"}' \
  localhost:50051 solid.v1.UserProvisioningService/RemoveDbUser
```

---

### 2. `solid.v1.DashboardService`

Manages SolidFlow dashboards (`dashboard` table) and personal publishing namespaces.

#### `CreateDashboard`
Creates a dashboard with widget definitions (`buttons_json`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "ops-main",
    "name": "Production Operations",
    "description": "Real-time service metrics and triggers",
    "owner": "usr_alice_01",
    "slug": "prod-ops",
    "is_public": true,
    "tags": ["engineering.backend"],
    "buttons_json": "[{\"id\":\"btn_health\",\"label\":\"Health Check\",\"type\":\"stat\",\"x\":0,\"y\":0,\"w\":4,\"h\":2}]"
  }' \
  localhost:50051 solid.v1.DashboardService/CreateDashboard
```

#### `GetDashboard`
Gets a dashboard by ID (`ops-main` or `dashboard:ops-main`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "ops-main"}' \
  localhost:50051 solid.v1.DashboardService/GetDashboard
```

#### `ListDashboards`
Lists dashboards with optional search query and `published_only` filter.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"query": "Production", "published_only": false}' \
  localhost:50051 solid.v1.DashboardService/ListDashboards
```

#### `UpdateDashboard`
Updates dashboard properties, tags, or widget layout (`buttons_json`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "ops-main",
    "name": "Production SRE Overview",
    "is_public": true,
    "tags": ["engineering.backend", "sre"],
    "update_tags": true
  }' \
  localhost:50051 solid.v1.DashboardService/UpdateDashboard
```

#### `DeleteDashboard`
Deletes a dashboard by ID.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "ops-main"}' \
  localhost:50051 solid.v1.DashboardService/DeleteDashboard
```

#### `SaveUserNamespace`
Claims or updates a user's unique publishing namespace and cascades the namespace to all dashboards owned by that user.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "sub_or_username": "alice",
    "namespace": "sre-team"
  }' \
  localhost:50051 solid.v1.DashboardService/SaveUserNamespace
```

---

### 3. `solid.v1.WorkflowService`

Manages SolidFlow workflows (`workflow` table), steps, DAG graphs, and workflow cloning.

#### `CreateWorkflow`
Creates a workflow with `steps_json` and `graph_json`.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "wf-nightly-sync",
    "name": "Nightly Order Sync",
    "description": "Fetches orders via gRPC and stores results in SurrealDB",
    "owner": "usr_alice_01",
    "schedule": "0 2 * * *",
    "server_address": "orders.internal:50051",
    "use_tls": false,
    "direction": "td",
    "visibility": "group",
    "tags": ["engineering.backend"],
    "steps_json": "[{\"id\":\"step_1\",\"name\":\"ListOrders\",\"stepType\":\"grpc\",\"service\":\"orders.v1.OrderService\",\"method\":\"ListOrders\",\"requestBody\":\"{}\"}]",
    "graph_json": "{\"nodes\":[{\"id\":\"step_1\",\"type\":\"grpc\"}],\"edges\":[]}"
  }' \
  localhost:50051 solid.v1.WorkflowService/CreateWorkflow
```

#### `GetWorkflow`
Gets a workflow by ID.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "wf-nightly-sync"}' \
  localhost:50051 solid.v1.WorkflowService/GetWorkflow
```

#### `ListWorkflows`
Lists workflows with optional search query.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"query": "Order"}' \
  localhost:50051 solid.v1.WorkflowService/ListWorkflows
```

#### `UpdateWorkflow`
Updates workflow steps, graph, schedule, or metadata.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "wf-nightly-sync",
    "schedule": "0 */4 * * *",
    "direction": "lr"
  }' \
  localhost:50051 solid.v1.WorkflowService/UpdateWorkflow
```

#### `CloneWorkflow`
Duplicates an existing workflow under a new ID and name (with schedule disabled by default).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "source_id": "wf-nightly-sync",
    "new_name": "Staging Order Sync",
    "owner": "usr_alice_01"
  }' \
  localhost:50051 solid.v1.WorkflowService/CloneWorkflow
```

#### `DeleteWorkflow`
Deletes a workflow by ID.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "wf-nightly-sync"}' \
  localhost:50051 solid.v1.WorkflowService/DeleteWorkflow
```

---

### 4. `solid.v1.ConnectionService`

Manages reusable SolidFlow connection profiles (`connection` table: `http`, `grpc`, `surrealdb`, `oauth`, `s3`).

#### `CreateConnection`
Creates a connection profile and merges protocol-specific properties onto the SurrealDB record.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "conn-orders-grpc",
    "name": "Orders gRPC Service",
    "conn_type": "grpc",
    "description": "Internal Orders gRPC endpoint",
    "owner": "usr_alice_01",
    "visibility": "group",
    "tags": ["engineering.backend"],
    "config_json": "{\"serverAddress\":\"orders.internal:50051\",\"useTls\":true,\"authType\":\"bearer\"}"
  }' \
  localhost:50051 solid.v1.ConnectionService/CreateConnection
```

#### `GetConnection`
Gets a connection profile by ID.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "conn-orders-grpc"}' \
  localhost:50051 solid.v1.ConnectionService/GetConnection
```

#### `ListConnections`
Lists connection profiles, optionally filtered by `conn_type` (`http`, `grpc`, `surrealdb`, `oauth`, `s3`).
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"conn_type": "grpc"}' \
  localhost:50051 solid.v1.ConnectionService/ListConnections
```

#### `UpdateConnection`
Updates a connection profile.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "conn-orders-grpc",
    "name": "Orders gRPC Service (v2)",
    "config_json": "{\"serverAddress\":\"orders-v2.internal:50051\",\"useTls\":true}"
  }' \
  localhost:50051 solid.v1.ConnectionService/UpdateConnection
```

#### `DeleteConnection`
Deletes a connection profile by ID.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "conn-orders-grpc"}' \
  localhost:50051 solid.v1.ConnectionService/DeleteConnection
```

---

### 5. `solid.v1.RegistryService`

Manages `.proto` files (`proto_file` table) and PEM-encoded Trusted CA Certificates (`ca_cert` table).

#### `UploadProto`
Uploads a `.proto` schema file to SurrealDB.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "orders-proto",
    "name": "orders.proto",
    "content": "syntax = \"proto3\";\npackage orders.v1;\nservice OrderService {}",
    "owner": "usr_alice_01",
    "visibility": "group",
    "tags": ["engineering.backend"]
  }' \
  localhost:50051 solid.v1.RegistryService/UploadProto
```

#### `GetProto`
Gets a `.proto` file by ID or file name.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "orders-proto"}' \
  localhost:50051 solid.v1.RegistryService/GetProto
```

#### `ListProtos`
Lists all `.proto` files in the registry.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"query": "orders"}' \
  localhost:50051 solid.v1.RegistryService/ListProtos
```

#### `UpdateProto`
Updates an existing `.proto` file's content, name, or tags.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "orders-proto",
    "visibility": "public"
  }' \
  localhost:50051 solid.v1.RegistryService/UpdateProto
```

#### `DeleteProto`
Deletes a `.proto` file from the registry.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "orders-proto"}' \
  localhost:50051 solid.v1.RegistryService/DeleteProto
```

#### `UploadCaCert`
Uploads a PEM-encoded Trusted Root CA Certificate to SurrealDB.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "corp-root-ca",
    "name": "Corp Internal Root CA",
    "content": "-----BEGIN CERTIFICATE-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A...\n-----END CERTIFICATE-----",
    "owner": "usr_alice_01",
    "visibility": "public",
    "tags": ["security"]
  }' \
  localhost:50051 solid.v1.RegistryService/UploadCaCert
```

#### `GetCaCert`
Gets a Trusted CA Certificate by ID or name.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "corp-root-ca"}' \
  localhost:50051 solid.v1.RegistryService/GetCaCert
```

#### `ListCaCerts`
Lists all Trusted CA Certificates.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{}' \
  localhost:50051 solid.v1.RegistryService/ListCaCerts
```

#### `UpdateCaCert`
Updates a Trusted CA Certificate.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "id": "corp-root-ca",
    "name": "Corp Root CA G2"
  }' \
  localhost:50051 solid.v1.RegistryService/UpdateCaCert
```

#### `DeleteCaCert`
Deletes a Trusted CA Certificate.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{"id": "corp-root-ca"}' \
  localhost:50051 solid.v1.RegistryService/DeleteCaCert
```

---

### 6. `solid.v1.DatabaseService`

Manages SurrealDB databases, `database_meta` space tags, and arbitrary SurrealQL query execution.

#### `CreateDatabase`
Defines a SurrealDB database (`DEFINE DATABASE IF NOT EXISTS ...`) and records its `database_meta` tags.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "name": "telemetry",
    "owner": "usr_alice_01",
    "tags": ["engineering.backend"]
  }' \
  localhost:50051 solid.v1.DatabaseService/CreateDatabase
```

#### `ListDatabases`
Lists databases in the namespace along with their `database_meta` tags.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{}' \
  localhost:50051 solid.v1.DatabaseService/ListDatabases
```

#### `UpdateDatabaseMeta`
Updates the owner or space tags for a database in `database_meta`.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "name": "telemetry",
    "owner": "usr_alice_01",
    "tags": ["engineering.backend", "observability"]
  }' \
  localhost:50051 solid.v1.DatabaseService/UpdateDatabaseMeta
```

#### `ExecuteQuery`
Executes parameterized SurrealQL queries and returns the JSON result.
```bash
grpcurl -plaintext -import-path proto -proto solid/v1/solid.proto \
  -d '{
    "query": "SELECT username, idpRole FROM user_profile WHERE idpRole = $role;",
    "vars_json": "{\"role\": \"admin\"}"
  }' \
  localhost:50051 solid.v1.DatabaseService/ExecuteQuery
```

---

## Docker (Distroless Runtime)

The [`Dockerfile`](Dockerfile) uses a multi-stage build with `rust:1-bookworm` as the builder and **`gcr.io/distroless/cc-debian12:nonroot`** as the minimal, hardened runtime image.

### Build Image

```bash
docker build -t solid-api:latest .
```

### Run Container

```bash
docker run --rm -p 50051:50051 \
  -e SURREALDB_URL="wss://app.ux-ti.com/rpc" \
  -e SURREALDB_NS="solidflow" \
  -e SURREALDB_DB="main" \
  -e SURREALDB_USER="root" \
  -e SURREALDB_PASS="root" \
  solid-api:latest
```
