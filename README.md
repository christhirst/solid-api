# Solid API — User & Role Provisioning Service for SurrealDB

`solid-api` is a high-performance backend microservice and CLI tool built in Rust (edition 2024) to manage and provision users, assign platform roles, manage hierarchical spaces/groups, and configure SurrealDB native database users.

It is designed to be fully compatible with the domain model and security architecture of **SolidFlow** (`solid.grpcfront`).

---

## Capabilities & Architecture

1. **User Provisioning & Profile Management (`user_profile`)**:
   - Provision users with subject ID (`sub`), normalized username, display name, email, and personal publishing namespace.
   - Platform roles: `admin`, `creator`, `user`, `viewer`.
   - Admin role override (`roleOverride`) support.
   - Effective access context resolution and capability calculation (`can_access_library`, `can_manage_workflows`, `can_manage_database`, etc.).

2. **Hierarchical Spaces / Groups & Rights (`group_namespace` & `group_member`)**:
   - Hierarchical group namespaces using dot notation (e.g. `engineering`, `engineering.backend`).
   - Group ownership and automatic manager rights assignment.
   - Granular space rights: `view`, `use`, `write`, `delete`, `manager`.
   - Downward inheritance & membership queries.

3. **SurrealDB Native Database Users**:
   - Provision SurrealDB database-level users (`DEFINE USER <username> ON DATABASE PASSWORD ... ROLES ...`).
   - Query and inspect defined database users via `INFO FOR DB;`.
   - Remove database users.

4. **Multi-Interface Access**:
   - **REST API** powered by [Axum](https://github.com/tokio-rs/axum).
   - **gRPC API** powered by [Tonic](https://github.com/hyperium/tonic) and Protocol Buffers (`proto/solid/v1/user_service.proto`).
   - **CLI Tool** powered by [Clap](https://github.com/clap-rs/clap).

---

## Configuration

Configuration is loaded from `config/default.toml` with layered overrides:
- `SURREALDB_URL`: SurrealDB endpoint (e.g. `wss://app.ux-ti.com/rpc`, `ws://localhost:8000/rpc`, or `memory`)
- `SURREALDB_NS`: Namespace (default: `solidflow`)
- `SURREALDB_DB`: Database (default: `main`)
- `SURREALDB_USER`: Username (default: `solid`)
- `SURREALDB_PASS`: Password (default: `sol1d`)
- `HTTP_PORT`: Port for the Axum REST API (default: `8080`)
- `GRPC_PORT`: Port for the Tonic gRPC service (default: `50052`)

---

## Running the Servers

Start both HTTP REST and gRPC servers concurrently:

```bash
cargo run -- serve --http-addr 0.0.0.0:8080 --grpc-addr 0.0.0.0:50052
```

---

## REST API Endpoints

### Health & Schema
- `GET /health`: Service health check.
- `POST /api/v1/schema/init`: Ensure SurrealDB tables and unique indexes are defined.

### User Management
- `POST /api/v1/users`: Provision a new user profile.
  ```json
  {
    "sub": "usr_123",
    "username": "alice",
    "name": "Alice Smith",
    "email": "alice@example.com",
    "role": "creator",
    "namespace": "alice-space"
  }
  ```
- `GET /api/v1/users`: List users (`?limit=100`).
- `GET /api/v1/users/{id_or_username}`: Get user profile details.
- `PUT /api/v1/users/{id_or_username}`: Update user profile.
- `POST /api/v1/users/{id_or_username}/role`: Assign platform role.
  ```json
  {
    "role": "admin",
    "as_override": false
  }
  ```
- `GET /api/v1/users/{id_or_username}/access`: Compute effective access context.
- `GET /api/v1/users/{id_or_username}/groups`: Get spaces assigned to user.
- `DELETE /api/v1/users/{id_or_username}`: Delete user and revoke memberships.

### Group / Space Management
- `POST /api/v1/groups`: Create space namespace.
  ```json
  {
    "name": "engineering.backend",
    "owner": "usr_123",
    "description": "Backend Team"
  }
  ```
- `GET /api/v1/groups`: List group namespaces.
- `GET /api/v1/groups/{name}`: Get space details.
- `DELETE /api/v1/groups/{name}`: Delete space and cascade members.
- `GET /api/v1/groups/{name}/members`: List space members.
- `POST /api/v1/groups/{name}/members`: Assign rights to user.
  ```json
  {
    "username": "bob",
    "rights": ["view", "use", "write"],
    "assigned_by": "usr_123"
  }
  ```
- `DELETE /api/v1/groups/{name}/members/{username}`: Remove user from space.

### SurrealDB Native Database Users
- `POST /api/v1/db-users`: Provision database-level user.
- `GET /api/v1/db-users`: List database users.
- `DELETE /api/v1/db-users/{username}`: Remove database user.

---

## CLI Usage

### Provisioning a User
```bash
solid-api user provision --username alice --role admin --name "Alice Smith" --email "alice@example.com"
```

### Inspecting User Details & Access
```bash
solid-api user get alice
solid-api user access alice
```

### Assigning a Platform Role
```bash
solid-api user assign-role alice --role creator
```

### Creating Spaces and Assigning Rights
```bash
# Create group
solid-api group create --name engineering.backend --owner alice

# Assign member rights
solid-api group assign --group engineering.backend --username bob --rights view,use,write

# List members
solid-api group members engineering.backend
```

### Native DB User Provisioning
```bash
solid-api db-user provision --username reports_service --password secret --roles VIEWER
```

---

## Running Tests

All integration and unit tests run in-memory without external dependencies:

```bash
cargo test
```
