use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;

fn default_idp_role() -> PlatformRole {
    PlatformRole::Viewer
}

fn default_visibility() -> String {
    "private".to_string()
}

fn default_direction() -> String {
    "td".to_string()
}

fn default_empty_array() -> serde_json::Value {
    serde_json::Value::Array(Vec::new())
}

fn default_empty_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, SurrealValue, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum PlatformRole {
    Admin,
    Creator,
    User,
    #[default]
    Viewer,
}

impl PlatformRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Creator => "creator",
            Self::User => "user",
            Self::Viewer => "viewer",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "admin" | "administrator" => Some(Self::Admin),
            "creator" => Some(Self::Creator),
            "user" => Some(Self::User),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            Self::Admin => 4,
            Self::Creator => 3,
            Self::User => 2,
            Self::Viewer => 1,
        }
    }

    pub fn capabilities(&self) -> PlatformCapabilities {
        match self {
            Self::Admin => PlatformCapabilities {
                can_access_library: true,
                can_edit_dashboards: true,
                can_manage_workflows: true,
                can_use_requests: true,
                can_manage_connections: true,
                can_manage_cas: true,
                can_manage_protos: true,
                can_manage_database: true,
                can_manage_groups: true,
            },
            Self::Creator => PlatformCapabilities {
                can_access_library: true,
                can_edit_dashboards: true,
                can_manage_workflows: true,
                can_use_requests: true,
                can_manage_connections: true,
                can_manage_cas: true,
                can_manage_protos: true,
                can_manage_database: false,
                can_manage_groups: true,
            },
            Self::User => PlatformCapabilities {
                can_access_library: true,
                can_edit_dashboards: true,
                can_manage_workflows: false,
                can_use_requests: false,
                can_manage_connections: false,
                can_manage_cas: false,
                can_manage_protos: false,
                can_manage_database: false,
                can_manage_groups: true,
            },
            Self::Viewer => PlatformCapabilities {
                can_access_library: true,
                can_edit_dashboards: false,
                can_manage_workflows: false,
                can_use_requests: false,
                can_manage_connections: false,
                can_manage_cas: false,
                can_manage_protos: false,
                can_manage_database: false,
                can_manage_groups: false,
            },
        }
    }
}

impl std::fmt::Display for PlatformRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, SurrealValue)]
#[serde(rename_all = "lowercase")]
pub enum GroupRight {
    View,
    Use,
    Write,
    Delete,
    Manager,
}

impl GroupRight {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Use => "use",
            Self::Write => "write",
            Self::Delete => "delete",
            Self::Manager => "manager",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "view" => Some(Self::View),
            "use" => Some(Self::Use),
            "write" => Some(Self::Write),
            "delete" => Some(Self::Delete),
            "manager" => Some(Self::Manager),
            _ => None,
        }
    }

    pub fn all_rights() -> Vec<Self> {
        vec![
            Self::View,
            Self::Use,
            Self::Write,
            Self::Delete,
            Self::Manager,
        ]
    }
}

impl std::fmt::Display for GroupRight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct PlatformCapabilities {
    pub can_access_library: bool,
    pub can_edit_dashboards: bool,
    pub can_manage_workflows: bool,
    pub can_use_requests: bool,
    pub can_manage_connections: bool,
    pub can_manage_cas: bool,
    pub can_manage_protos: bool,
    pub can_manage_database: bool,
    pub can_manage_groups: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct UserProfile {
    pub sub: String,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "idpRole", default = "default_idp_role")]
    pub idp_role: PlatformRole,
    #[serde(
        rename = "roleOverride",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub role_override: Option<PlatformRole>,
    #[serde(
        rename = "simulatedRole",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub simulated_role: Option<PlatformRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "groupGrants", default)]
    pub group_grants: Vec<String>,
    #[serde(rename = "disabledGroups", default)]
    pub disabled_groups: Vec<String>,
    #[serde(rename = "disableOwnerBypass", default)]
    pub disable_owner_bypass: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct GroupNamespace {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub owner: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct GroupMember {
    pub group: String,
    pub username: String,
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub display_name: Option<String>,
    #[serde(rename = "userSub", default, skip_serializing_if = "Option::is_none")]
    pub user_sub: Option<String>,
    #[serde(default)]
    pub rights: Vec<GroupRight>,
    #[serde(rename = "assignedBy", default)]
    pub assigned_by: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct DbUserRecord {
    pub username: String,
    pub roles: Vec<String>,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct EffectiveAccessContext {
    pub sub: String,
    pub username: String,
    pub email: Option<String>,
    pub idp_role: PlatformRole,
    pub platform_role: PlatformRole,
    pub simulated_role: Option<PlatformRole>,
    pub capabilities: PlatformCapabilities,
    pub group_grants: Vec<String>,
    pub is_admin: bool,
}

// ============================================================================
// SolidFlow Resource Records (Dashboards, Workflows, Connections, Registry, DB)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct DashboardRecord {
    #[serde(default)]
    pub record_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(rename = "isPublic", default)]
    pub is_public: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_empty_array")]
    pub buttons: serde_json::Value,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct WorkflowRecord {
    #[serde(default)]
    pub record_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(
        rename = "serverAddress",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub server_address: Option<String>,
    #[serde(rename = "useTls", default)]
    pub use_tls: bool,
    #[serde(default = "default_direction")]
    pub direction: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_empty_array")]
    pub steps: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct ConnectionRecord {
    #[serde(default)]
    pub record_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub conn_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_empty_object")]
    pub config: serde_json::Value,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct ProtoFileRecord {
    #[serde(default)]
    pub record_id: String,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct CaCertRecord {
    #[serde(default)]
    pub record_id: String,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct DatabaseMetaRecord {
    pub name: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

// ============================================================================
// Request Payloads
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ProvisionUserRequest {
    pub sub: Option<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub role: Option<PlatformRole>,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub namespace: Option<String>,
    pub role: Option<PlatformRole>,
    pub role_override: Option<Option<PlatformRole>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssignPlatformRoleRequest {
    pub role: PlatformRole,
    pub as_override: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub owner: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssignGroupMemberRequest {
    pub username: String,
    #[serde(default)]
    pub rights: Vec<GroupRight>,
    pub assigned_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProvisionDbUserRequest {
    pub username: String,
    pub password: String,
    pub roles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDashboardInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub slug: Option<String>,
    pub is_public: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    pub buttons_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateDashboardInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub slug: Option<String>,
    pub is_public: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub buttons_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorkflowInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub schedule: Option<String>,
    pub server_address: Option<String>,
    pub use_tls: bool,
    pub direction: Option<String>,
    pub visibility: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub steps_json: Option<String>,
    pub graph_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateWorkflowInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub schedule: Option<String>,
    pub server_address: Option<String>,
    pub use_tls: Option<bool>,
    pub direction: Option<String>,
    pub visibility: Option<String>,
    pub tags: Option<Vec<String>>,
    pub steps_json: Option<String>,
    pub graph_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateConnectionInput {
    pub id: Option<String>,
    pub name: String,
    pub conn_type: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub visibility: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub config_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateConnectionInput {
    pub name: Option<String>,
    pub conn_type: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub visibility: Option<String>,
    pub tags: Option<Vec<String>>,
    pub config_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadProtoInput {
    pub id: Option<String>,
    pub name: String,
    pub content: String,
    pub owner: Option<String>,
    pub visibility: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateProtoInput {
    pub name: Option<String>,
    pub content: Option<String>,
    pub owner: Option<String>,
    pub visibility: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadCaCertInput {
    pub id: Option<String>,
    pub name: String,
    pub content: String,
    pub owner: Option<String>,
    pub visibility: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateCaCertInput {
    pub name: Option<String>,
    pub content: Option<String>,
    pub owner: Option<String>,
    pub visibility: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDatabaseInput {
    pub name: String,
    pub owner: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateDatabaseMetaInput {
    pub owner: Option<String>,
    pub tags: Option<Vec<String>>,
}
