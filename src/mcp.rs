use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::error::AppError;
use crate::models::{
    AssignGroupMemberRequest, CreateConnectionInput, CreateDashboardInput, CreateDatabaseInput,
    CreateGroupRequest, CreateWorkflowInput, GroupRight, PlatformRole, ProvisionDbUserRequest,
    ProvisionUserRequest, UpdateConnectionInput, UpdateDashboardInput, UpdateWorkflowInput,
    UploadCaCertInput, UploadProtoInput,
};
use crate::schema::ensure_schema;
use crate::store::Store;

/// Runs the Model Context Protocol (MCP) JSON-RPC 2.0 server over stdio.
pub async fn run_mcp_stdio(store: Store) -> Result<(), AppError> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| AppError::Internal(format!("MCP stdin read error: {e}")))?
    {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req_val: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {e}")
                    }
                });
                let mut out = serde_json::to_string(&err_resp)?;
                out.push('\n');
                let _ = stdout.write_all(out.as_bytes()).await;
                let _ = stdout.flush().await;
                continue;
            }
        };

        if let Some(resp_val) = handle_mcp_request(&store, req_val).await {
            let mut out = serde_json::to_string(&resp_val)?;
            out.push('\n');
            stdout
                .write_all(out.as_bytes())
                .await
                .map_err(|e| AppError::Internal(format!("MCP stdout write error: {e}")))?;
            stdout
                .flush()
                .await
                .map_err(|e| AppError::Internal(format!("MCP stdout flush error: {e}")))?;
        }
    }

    Ok(())
}

/// Processes a single MCP JSON-RPC 2.0 request and returns a JSON-RPC response
/// (`None` for notifications that do not expect a response).
pub async fn handle_mcp_request(store: &Store, req: Value) -> Option<Value> {
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

    // JSON-RPC notifications have no "id"
    if id.is_none() || method.starts_with("notifications/") {
        return None;
    }

    let id_val = id.unwrap_or(Value::Null);

    match method {
        "initialize" => Some(json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "solid-api-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        })),
        "ping" => Some(json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "result": {}
        })),
        "tools/list" => Some(json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "result": {
                "tools": mcp_tools_catalog()
            }
        })),
        "tools/call" => {
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            match execute_tool(store, tool_name, args).await {
                Ok(result_value) => {
                    let text =
                        serde_json::to_string_pretty(&result_value).unwrap_or_else(|_| "{}".into());
                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": text
                                }
                            ],
                            "isError": false
                        }
                    }))
                }
                Err(err) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id_val,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Error: {err}")
                            }
                        ],
                        "isError": true
                    }
                })),
            }
        }
        _ => Some(json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "error": {
                "code": -32601,
                "message": format!("Method not found: {method}")
            }
        })),
    }
}

fn str_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn req_str_arg(args: &Value, key: &str) -> Result<String, AppError> {
    str_arg(args, key)
        .ok_or_else(|| AppError::InvalidArgument(format!("Missing required argument '{key}'")))
}

fn bool_arg(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn opt_bool_arg(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn string_vec_arg(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn opt_string_vec_arg(args: &Value, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|item| item.as_str().map(|s| s.to_string()))
            .collect()
    })
}

fn json_string_arg(args: &Value, key: &str) -> Option<String> {
    match args.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        Some(v) if !v.is_null() => serde_json::to_string(v).ok(),
        _ => None,
    }
}

async fn execute_tool(store: &Store, name: &str, args: Value) -> Result<Value, AppError> {
    match name {
        // Schema
        "init_schema" => {
            ensure_schema(&store.db).await?;
            Ok(json!({ "success": true, "message": "Schema initialized" }))
        }

        // Users & Roles
        "provision_user" => {
            let username = req_str_arg(&args, "username")?;
            let role = str_arg(&args, "role")
                .map(|r| {
                    PlatformRole::from_str_loose(&r).ok_or_else(|| {
                        AppError::InvalidArgument(format!("Invalid platform role '{r}'"))
                    })
                })
                .transpose()?;

            let user = store
                .provision_user(ProvisionUserRequest {
                    sub: str_arg(&args, "sub"),
                    username,
                    name: str_arg(&args, "name"),
                    email: str_arg(&args, "email"),
                    role,
                    namespace: str_arg(&args, "namespace"),
                })
                .await?;
            Ok(serde_json::to_value(user)?)
        }
        "get_user" => {
            let sub_or_username = req_str_arg(&args, "sub_or_username")?;
            let user = store.get_user(&sub_or_username).await?;
            Ok(serde_json::to_value(user)?)
        }
        "list_users" => {
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            let users = store.list_users(limit).await?;
            Ok(serde_json::to_value(users)?)
        }
        "assign_platform_role" => {
            let sub_or_username = req_str_arg(&args, "sub_or_username")?;
            let role_str = req_str_arg(&args, "role")?;
            let role = PlatformRole::from_str_loose(&role_str).ok_or_else(|| {
                AppError::InvalidArgument(format!("Invalid platform role '{role_str}'"))
            })?;
            let as_override = bool_arg(&args, "as_override", false);
            let user = store
                .update_user_role(&sub_or_username, role, as_override)
                .await?;
            Ok(serde_json::to_value(user)?)
        }
        "delete_user" => {
            let sub_or_username = req_str_arg(&args, "sub_or_username")?;
            let deleted = store.delete_user(&sub_or_username).await?;
            Ok(json!({ "deleted": deleted }))
        }
        "get_effective_access" => {
            let sub_or_username = req_str_arg(&args, "sub_or_username")?;
            let access = store.get_effective_access(&sub_or_username).await?;
            Ok(serde_json::to_value(access)?)
        }

        // Groups / Spaces
        "create_group" => {
            let name = req_str_arg(&args, "name")?;
            let owner = str_arg(&args, "owner").unwrap_or_else(|| "system".to_string());
            let group = store
                .create_group(CreateGroupRequest {
                    name,
                    owner,
                    description: str_arg(&args, "description"),
                })
                .await?;
            Ok(serde_json::to_value(group)?)
        }
        "list_groups" => {
            let groups = store.list_groups().await?;
            Ok(serde_json::to_value(groups)?)
        }
        "assign_group_member" => {
            let group_name = req_str_arg(&args, "group_name")?;
            let username = req_str_arg(&args, "username")?;
            let rights: Vec<GroupRight> = string_vec_arg(&args, "rights")
                .iter()
                .filter_map(|r| GroupRight::from_str_loose(r))
                .collect();
            let member = store
                .assign_group_role(
                    &group_name,
                    AssignGroupMemberRequest {
                        username,
                        rights,
                        assigned_by: str_arg(&args, "assigned_by"),
                    },
                )
                .await?;
            Ok(serde_json::to_value(member)?)
        }
        "remove_group_member" => {
            let group_name = req_str_arg(&args, "group_name")?;
            let username = req_str_arg(&args, "username")?;
            let removed = store.remove_group_member(&group_name, &username).await?;
            Ok(json!({ "removed": removed }))
        }

        // Native SurrealDB DB Users
        "provision_db_user" => {
            let username = req_str_arg(&args, "username")?;
            let password = req_str_arg(&args, "password")?;
            let roles = opt_string_vec_arg(&args, "roles");
            let db_user = store
                .provision_db_user(ProvisionDbUserRequest {
                    username,
                    password,
                    roles,
                })
                .await?;
            Ok(serde_json::to_value(db_user)?)
        }
        "list_db_users" => {
            let users = store.list_db_users().await?;
            Ok(serde_json::to_value(users)?)
        }

        // Dashboards
        "create_dashboard" => {
            let name = req_str_arg(&args, "name")?;
            let dashboard = store
                .create_dashboard(CreateDashboardInput {
                    id: str_arg(&args, "id"),
                    name,
                    description: str_arg(&args, "description"),
                    owner: str_arg(&args, "owner"),
                    slug: str_arg(&args, "slug"),
                    is_public: bool_arg(&args, "is_public", false),
                    tags: string_vec_arg(&args, "tags"),
                    buttons_json: json_string_arg(&args, "buttons"),
                })
                .await?;
            Ok(serde_json::to_value(dashboard)?)
        }
        "get_dashboard" => {
            let id = req_str_arg(&args, "id")?;
            let dashboard = store.get_dashboard(&id).await?;
            Ok(serde_json::to_value(dashboard)?)
        }
        "list_dashboards" => {
            let query = str_arg(&args, "query");
            let published_only = bool_arg(&args, "published_only", false);
            let dashboards = store
                .list_dashboards(query.as_deref(), published_only)
                .await?;
            Ok(serde_json::to_value(dashboards)?)
        }
        "update_dashboard" => {
            let id = req_str_arg(&args, "id")?;
            let dashboard = store
                .update_dashboard(
                    &id,
                    UpdateDashboardInput {
                        name: str_arg(&args, "name"),
                        description: str_arg(&args, "description"),
                        owner: str_arg(&args, "owner"),
                        slug: str_arg(&args, "slug"),
                        is_public: opt_bool_arg(&args, "is_public"),
                        tags: opt_string_vec_arg(&args, "tags"),
                        buttons_json: json_string_arg(&args, "buttons"),
                    },
                )
                .await?;
            Ok(serde_json::to_value(dashboard)?)
        }
        "delete_dashboard" => {
            let id = req_str_arg(&args, "id")?;
            let deleted = store.delete_dashboard(&id).await?;
            Ok(json!({ "deleted": deleted }))
        }
        "save_user_namespace" => {
            let sub_or_username = req_str_arg(&args, "sub_or_username")?;
            let namespace = req_str_arg(&args, "namespace")?;
            let (sub, ns) = store
                .save_user_namespace(&sub_or_username, &namespace)
                .await?;
            Ok(json!({ "sub": sub, "namespace": ns }))
        }

        // Workflows
        "create_workflow" => {
            let name = req_str_arg(&args, "name")?;
            let workflow = store
                .create_workflow(CreateWorkflowInput {
                    id: str_arg(&args, "id"),
                    name,
                    description: str_arg(&args, "description"),
                    owner: str_arg(&args, "owner"),
                    schedule: str_arg(&args, "schedule"),
                    server_address: str_arg(&args, "server_address"),
                    use_tls: bool_arg(&args, "use_tls", false),
                    direction: str_arg(&args, "direction"),
                    visibility: str_arg(&args, "visibility"),
                    tags: string_vec_arg(&args, "tags"),
                    steps_json: json_string_arg(&args, "steps"),
                    graph_json: json_string_arg(&args, "graph"),
                })
                .await?;
            Ok(serde_json::to_value(workflow)?)
        }
        "get_workflow" => {
            let id = req_str_arg(&args, "id")?;
            let workflow = store.get_workflow(&id).await?;
            Ok(serde_json::to_value(workflow)?)
        }
        "list_workflows" => {
            let query = str_arg(&args, "query");
            let workflows = store.list_workflows(query.as_deref()).await?;
            Ok(serde_json::to_value(workflows)?)
        }
        "update_workflow" => {
            let id = req_str_arg(&args, "id")?;
            let workflow = store
                .update_workflow(
                    &id,
                    UpdateWorkflowInput {
                        name: str_arg(&args, "name"),
                        description: str_arg(&args, "description"),
                        owner: str_arg(&args, "owner"),
                        schedule: str_arg(&args, "schedule"),
                        server_address: str_arg(&args, "server_address"),
                        use_tls: opt_bool_arg(&args, "use_tls"),
                        direction: str_arg(&args, "direction"),
                        visibility: str_arg(&args, "visibility"),
                        tags: opt_string_vec_arg(&args, "tags"),
                        steps_json: json_string_arg(&args, "steps"),
                        graph_json: json_string_arg(&args, "graph"),
                    },
                )
                .await?;
            Ok(serde_json::to_value(workflow)?)
        }
        "delete_workflow" => {
            let id = req_str_arg(&args, "id")?;
            let deleted = store.delete_workflow(&id).await?;
            Ok(json!({ "deleted": deleted }))
        }
        "clone_workflow" => {
            let source_id = req_str_arg(&args, "source_id")?;
            let workflow = store
                .clone_workflow(
                    &source_id,
                    str_arg(&args, "new_name"),
                    str_arg(&args, "owner"),
                )
                .await?;
            Ok(serde_json::to_value(workflow)?)
        }

        // Connections
        "create_connection" => {
            let name = req_str_arg(&args, "name")?;
            let conn_type = req_str_arg(&args, "conn_type")?;
            let conn = store
                .create_connection(CreateConnectionInput {
                    id: str_arg(&args, "id"),
                    name,
                    conn_type,
                    description: str_arg(&args, "description"),
                    owner: str_arg(&args, "owner"),
                    visibility: str_arg(&args, "visibility"),
                    tags: string_vec_arg(&args, "tags"),
                    config_json: json_string_arg(&args, "config"),
                })
                .await?;
            Ok(serde_json::to_value(conn)?)
        }
        "get_connection" => {
            let id = req_str_arg(&args, "id")?;
            let conn = store.get_connection(&id).await?;
            Ok(serde_json::to_value(conn)?)
        }
        "list_connections" => {
            let conn_type = str_arg(&args, "conn_type");
            let conns = store.list_connections(conn_type.as_deref()).await?;
            Ok(serde_json::to_value(conns)?)
        }
        "update_connection" => {
            let id = req_str_arg(&args, "id")?;
            let conn = store
                .update_connection(
                    &id,
                    UpdateConnectionInput {
                        name: str_arg(&args, "name"),
                        conn_type: str_arg(&args, "conn_type"),
                        description: str_arg(&args, "description"),
                        owner: str_arg(&args, "owner"),
                        visibility: str_arg(&args, "visibility"),
                        tags: opt_string_vec_arg(&args, "tags"),
                        config_json: json_string_arg(&args, "config"),
                    },
                )
                .await?;
            Ok(serde_json::to_value(conn)?)
        }
        "delete_connection" => {
            let id = req_str_arg(&args, "id")?;
            let deleted = store.delete_connection(&id).await?;
            Ok(json!({ "deleted": deleted }))
        }

        // Proto Registry & Trusted CA Certificates
        "upload_proto" => {
            let name = req_str_arg(&args, "name")?;
            let content = req_str_arg(&args, "content")?;
            let proto = store
                .upload_proto(UploadProtoInput {
                    id: str_arg(&args, "id"),
                    name,
                    content,
                    owner: str_arg(&args, "owner"),
                    visibility: str_arg(&args, "visibility"),
                    tags: string_vec_arg(&args, "tags"),
                })
                .await?;
            Ok(serde_json::to_value(proto)?)
        }
        "get_proto" => {
            let id = req_str_arg(&args, "id")?;
            let proto = store.get_proto(&id).await?;
            Ok(serde_json::to_value(proto)?)
        }
        "list_protos" => {
            let query = str_arg(&args, "query");
            let protos = store.list_protos(query.as_deref()).await?;
            Ok(serde_json::to_value(protos)?)
        }
        "delete_proto" => {
            let id = req_str_arg(&args, "id")?;
            let deleted = store.delete_proto(&id).await?;
            Ok(json!({ "deleted": deleted }))
        }
        "upload_ca_cert" => {
            let name = req_str_arg(&args, "name")?;
            let content = req_str_arg(&args, "content")?;
            let cert = store
                .upload_ca_cert(UploadCaCertInput {
                    id: str_arg(&args, "id"),
                    name,
                    content,
                    owner: str_arg(&args, "owner"),
                    visibility: str_arg(&args, "visibility"),
                    tags: string_vec_arg(&args, "tags"),
                })
                .await?;
            Ok(serde_json::to_value(cert)?)
        }
        "get_ca_cert" => {
            let id = req_str_arg(&args, "id")?;
            let cert = store.get_ca_cert(&id).await?;
            Ok(serde_json::to_value(cert)?)
        }
        "list_ca_certs" => {
            let certs = store.list_ca_certs().await?;
            Ok(serde_json::to_value(certs)?)
        }
        "delete_ca_cert" => {
            let id = req_str_arg(&args, "id")?;
            let deleted = store.delete_ca_cert(&id).await?;
            Ok(json!({ "deleted": deleted }))
        }

        // Databases & SurrealQL
        "list_databases" => {
            let dbs = store.list_databases().await?;
            Ok(serde_json::to_value(dbs)?)
        }
        "create_database" => {
            let name = req_str_arg(&args, "name")?;
            let db = store
                .create_database(CreateDatabaseInput {
                    name,
                    owner: str_arg(&args, "owner"),
                    tags: string_vec_arg(&args, "tags"),
                })
                .await?;
            Ok(serde_json::to_value(db)?)
        }
        "execute_surreal_query" => {
            let query = req_str_arg(&args, "query")?;
            let vars = json_string_arg(&args, "vars");
            let res = store.execute_query(&query, vars.as_deref()).await?;
            Ok(res)
        }

        _ => Err(AppError::InvalidArgument(format!(
            "Unknown MCP tool '{name}'"
        ))),
    }
}

fn mcp_tools_catalog() -> Value {
    json!([
        {
            "name": "init_schema",
            "description": "Initialize all SolidFlow SurrealDB tables and unique indexes",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "provision_user",
            "description": "Provision a new user profile in SurrealDB with a platform role (admin, creator, user, viewer)",
            "inputSchema": {
                "type": "object",
                "required": ["username"],
                "properties": {
                    "sub": { "type": "string", "description": "Subject ID (UUID generated if omitted)" },
                    "username": { "type": "string", "description": "Unique username" },
                    "name": { "type": "string", "description": "Display name" },
                    "email": { "type": "string", "description": "Email address" },
                    "role": { "type": "string", "enum": ["admin", "creator", "user", "viewer"] },
                    "namespace": { "type": "string", "description": "Personal publishing namespace" }
                }
            }
        },
        {
            "name": "get_user",
            "description": "Get a user profile by sub, username, or email",
            "inputSchema": {
                "type": "object",
                "required": ["sub_or_username"],
                "properties": {
                    "sub_or_username": { "type": "string" }
                }
            }
        },
        {
            "name": "list_users",
            "description": "List registered SolidFlow user profiles",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max users to return" }
                }
            }
        },
        {
            "name": "assign_platform_role",
            "description": "Assign or override a user's SolidFlow platform role (admin, creator, user, viewer)",
            "inputSchema": {
                "type": "object",
                "required": ["sub_or_username", "role"],
                "properties": {
                    "sub_or_username": { "type": "string" },
                    "role": { "type": "string", "enum": ["admin", "creator", "user", "viewer"] },
                    "as_override": { "type": "boolean" }
                }
            }
        },
        {
            "name": "delete_user",
            "description": "Delete a user profile and clean up group memberships",
            "inputSchema": {
                "type": "object",
                "required": ["sub_or_username"],
                "properties": {
                    "sub_or_username": { "type": "string" }
                }
            }
        },
        {
            "name": "get_effective_access",
            "description": "Compute effective platform role, capabilities, and group grants for a user",
            "inputSchema": {
                "type": "object",
                "required": ["sub_or_username"],
                "properties": {
                    "sub_or_username": { "type": "string" }
                }
            }
        },
        {
            "name": "create_group",
            "description": "Create a hierarchical space / group namespace (e.g. engineering.backend)",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "owner": { "type": "string" },
                    "description": { "type": "string" }
                }
            }
        },
        {
            "name": "list_groups",
            "description": "List all hierarchical spaces / group namespaces",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "assign_group_member",
            "description": "Assign a user to a group with specific rights (view, use, write, delete, manager)",
            "inputSchema": {
                "type": "object",
                "required": ["group_name", "username"],
                "properties": {
                    "group_name": { "type": "string" },
                    "username": { "type": "string" },
                    "rights": { "type": "array", "items": { "type": "string" } },
                    "assigned_by": { "type": "string" }
                }
            }
        },
        {
            "name": "remove_group_member",
            "description": "Remove a user from a group namespace",
            "inputSchema": {
                "type": "object",
                "required": ["group_name", "username"],
                "properties": {
                    "group_name": { "type": "string" },
                    "username": { "type": "string" }
                }
            }
        },
        {
            "name": "provision_db_user",
            "description": "Provision a native SurrealDB database-level user (DEFINE USER ... ON DATABASE)",
            "inputSchema": {
                "type": "object",
                "required": ["username", "password"],
                "properties": {
                    "username": { "type": "string" },
                    "password": { "type": "string" },
                    "roles": { "type": "array", "items": { "type": "string" } }
                }
            }
        },
        {
            "name": "list_db_users",
            "description": "List native SurrealDB database users",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "create_dashboard",
            "description": "Create a SolidFlow dashboard with widgets/buttons",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "owner": { "type": "string" },
                    "slug": { "type": "string" },
                    "is_public": { "type": "boolean" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "buttons": { "description": "JSON array of dashboard widget objects or JSON string" }
                }
            }
        },
        {
            "name": "get_dashboard",
            "description": "Get a SolidFlow dashboard by ID",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "list_dashboards",
            "description": "List SolidFlow dashboards with optional search query",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "published_only": { "type": "boolean" }
                }
            }
        },
        {
            "name": "update_dashboard",
            "description": "Update an existing SolidFlow dashboard",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "owner": { "type": "string" },
                    "slug": { "type": "string" },
                    "is_public": { "type": "boolean" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "buttons": { "description": "JSON array of dashboard widget objects or JSON string" }
                }
            }
        },
        {
            "name": "delete_dashboard",
            "description": "Delete a SolidFlow dashboard by ID",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "save_user_namespace",
            "description": "Claim or update a user's personal publishing namespace and sync their dashboards",
            "inputSchema": {
                "type": "object",
                "required": ["sub_or_username", "namespace"],
                "properties": {
                    "sub_or_username": { "type": "string" },
                    "namespace": { "type": "string" }
                }
            }
        },
        {
            "name": "create_workflow",
            "description": "Create a SolidFlow workflow (steps + DAG graph)",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "owner": { "type": "string" },
                    "schedule": { "type": "string" },
                    "server_address": { "type": "string" },
                    "use_tls": { "type": "boolean" },
                    "direction": { "type": "string", "enum": ["td", "lr"] },
                    "visibility": { "type": "string", "enum": ["private", "group", "public"] },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "steps": { "description": "JSON array of workflow steps or JSON string" },
                    "graph": { "description": "JSON object with nodes and edges or JSON string" }
                }
            }
        },
        {
            "name": "get_workflow",
            "description": "Get a SolidFlow workflow by ID",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "list_workflows",
            "description": "List SolidFlow workflows",
            "inputSchema": {
                "type": "object",
                "properties": { "query": { "type": "string" } }
            }
        },
        {
            "name": "update_workflow",
            "description": "Update an existing SolidFlow workflow",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "owner": { "type": "string" },
                    "schedule": { "type": "string" },
                    "server_address": { "type": "string" },
                    "use_tls": { "type": "boolean" },
                    "direction": { "type": "string" },
                    "visibility": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "steps": { "description": "JSON array of workflow steps or JSON string" },
                    "graph": { "description": "JSON object with nodes and edges or JSON string" }
                }
            }
        },
        {
            "name": "delete_workflow",
            "description": "Delete a SolidFlow workflow by ID",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "clone_workflow",
            "description": "Clone an existing SolidFlow workflow with a new ID and name",
            "inputSchema": {
                "type": "object",
                "required": ["source_id"],
                "properties": {
                    "source_id": { "type": "string" },
                    "new_name": { "type": "string" },
                    "owner": { "type": "string" }
                }
            }
        },
        {
            "name": "create_connection",
            "description": "Create a reusable SolidFlow connection profile (http, grpc, surrealdb, oauth, s3)",
            "inputSchema": {
                "type": "object",
                "required": ["name", "conn_type"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "conn_type": { "type": "string", "enum": ["http", "grpc", "surrealdb", "oauth", "s3"] },
                    "description": { "type": "string" },
                    "owner": { "type": "string" },
                    "visibility": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "config": { "description": "Protocol-specific configuration object or JSON string" }
                }
            }
        },
        {
            "name": "get_connection",
            "description": "Get a SolidFlow connection profile by ID",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "list_connections",
            "description": "List SolidFlow connection profiles with optional type filter",
            "inputSchema": {
                "type": "object",
                "properties": { "conn_type": { "type": "string" } }
            }
        },
        {
            "name": "update_connection",
            "description": "Update an existing SolidFlow connection profile",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "conn_type": { "type": "string" },
                    "description": { "type": "string" },
                    "owner": { "type": "string" },
                    "visibility": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "config": { "description": "Protocol-specific configuration object or JSON string" }
                }
            }
        },
        {
            "name": "delete_connection",
            "description": "Delete a SolidFlow connection profile by ID",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "upload_proto",
            "description": "Upload a .proto file to the SolidFlow Proto Registry in SurrealDB",
            "inputSchema": {
                "type": "object",
                "required": ["name", "content"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "content": { "type": "string" },
                    "owner": { "type": "string" },
                    "visibility": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } }
                }
            }
        },
        {
            "name": "get_proto",
            "description": "Get a .proto file by ID or name",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "list_protos",
            "description": "List .proto files in the SolidFlow Proto Registry",
            "inputSchema": {
                "type": "object",
                "properties": { "query": { "type": "string" } }
            }
        },
        {
            "name": "delete_proto",
            "description": "Delete a .proto file from the SolidFlow Proto Registry",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "upload_ca_cert",
            "description": "Upload a PEM-encoded Trusted CA Certificate to SurrealDB",
            "inputSchema": {
                "type": "object",
                "required": ["name", "content"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "content": { "type": "string" },
                    "owner": { "type": "string" },
                    "visibility": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } }
                }
            }
        },
        {
            "name": "get_ca_cert",
            "description": "Get a Trusted CA Certificate by ID or name",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "list_ca_certs",
            "description": "List all Trusted CA Certificates in SurrealDB",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "delete_ca_cert",
            "description": "Delete a Trusted CA Certificate by ID",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "list_databases",
            "description": "List SurrealDB databases and their metadata/tags",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "create_database",
            "description": "Define a SurrealDB database and store its database_meta tags",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "owner": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } }
                }
            }
        },
        {
            "name": "execute_surreal_query",
            "description": "Execute arbitrary SurrealQL against the active SurrealDB namespace/database",
            "inputSchema": {
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string" },
                    "vars": { "description": "Optional bind variables object or JSON string" }
                }
            }
        }
    ])
}
