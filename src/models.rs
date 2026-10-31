use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;

fn default_idp_role() -> PlatformRole {
    PlatformRole::Viewer
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

// Request and Response payloads

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
