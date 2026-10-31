use chrono::Utc;
use surrealdb::Surreal;
use surrealdb::engine::any::{Any, connect};
use surrealdb::opt::auth::{Database, Root};
use uuid::Uuid;

use crate::config::SurrealDbConfig;
use crate::error::AppError;
use crate::models::{
    AssignGroupMemberRequest, CreateGroupRequest, DbUserRecord, EffectiveAccessContext,
    GroupMember, GroupNamespace, GroupRight, PlatformRole, ProvisionDbUserRequest,
    ProvisionUserRequest, UpdateUserRequest, UserProfile,
};
use crate::schema::ensure_schema;

#[derive(Debug, Clone)]
pub struct Store {
    pub db: Surreal<Any>,
}

impl Store {
    pub async fn connect(cfg: &SurrealDbConfig) -> Result<Self, AppError> {
        let db = connect(&cfg.endpoint).await?;

        if let (Some(username), Some(password)) = (&cfg.username, &cfg.password)
            && !username.is_empty()
        {
            if username == "root" {
                let _ = db
                    .signin(Root {
                        username: username.clone(),
                        password: password.clone(),
                    })
                    .await;
            } else {
                // Try scoped Database auth first, then fallback to Root
                let db_auth = db
                    .signin(Database {
                        namespace: cfg.namespace.clone(),
                        database: cfg.database.clone(),
                        username: username.clone(),
                        password: password.clone(),
                    })
                    .await;

                if db_auth.is_err() {
                    let _ = db
                        .signin(Root {
                            username: username.clone(),
                            password: password.clone(),
                        })
                        .await;
                }
            }
        }

        db.use_ns(&cfg.namespace).use_db(&cfg.database).await?;

        // Initialize table schemas and unique indexes
        let _ = ensure_schema(&db).await;

        Ok(Self { db })
    }

    pub async fn new_in_memory() -> Result<Self, AppError> {
        let db = connect("memory").await?;
        db.use_ns("solidflow").use_db("main").await?;
        ensure_schema(&db).await?;
        Ok(Self { db })
    }

    fn now_iso() -> String {
        Utc::now().to_rfc3339()
    }

    fn normalize_name(s: &str) -> String {
        s.trim().to_lowercase()
    }

    fn member_record_key(group: &str, username: &str) -> String {
        format!(
            "{}__{}",
            Self::normalize_name(group),
            Self::normalize_name(username)
        )
    }

    // ========================================================================
    // User Operations
    // ========================================================================

    pub async fn provision_user(&self, req: ProvisionUserRequest) -> Result<UserProfile, AppError> {
        let clean_username = Self::normalize_name(&req.username);
        if clean_username.is_empty() {
            return Err(AppError::InvalidArgument("Username cannot be empty".into()));
        }

        let sub = req
            .sub
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Check if a user with this sub or username already exists
        let existing: Option<UserProfile> = self.db.select(("user_profile", sub.as_str())).await?;
        if existing.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "User with sub '{sub}' already exists"
            )));
        }

        let mut check_resp = self
            .db
            .query("SELECT * FROM user_profile WHERE username = $username LIMIT 1;")
            .bind(("username", clean_username.clone()))
            .await?;
        let check_user: Vec<UserProfile> = check_resp.take(0)?;
        if !check_user.is_empty() {
            return Err(AppError::AlreadyExists(format!(
                "User with username '{clean_username}' already exists"
            )));
        }

        let now = Self::now_iso();
        let idp_role = req.role.unwrap_or(PlatformRole::Viewer);

        let user = UserProfile {
            sub: sub.clone(),
            username: clean_username,
            name: req
                .name
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty()),
            email: req
                .email
                .map(|e| e.trim().to_string())
                .filter(|e| !e.is_empty()),
            idp_role,
            role_override: None,
            simulated_role: None,
            namespace: req
                .namespace
                .map(|ns| ns.trim().to_string())
                .filter(|ns| !ns.is_empty()),
            group_grants: Vec::new(),
            disabled_groups: Vec::new(),
            disable_owner_bypass: false,
            created_at: Some(now.clone()),
            updated_at: now,
        };

        let created: Option<UserProfile> = self
            .db
            .upsert(("user_profile", sub.as_str()))
            .content(user.clone())
            .await?;

        Ok(created.unwrap_or(user))
    }

    pub async fn get_user(&self, sub_or_username: &str) -> Result<UserProfile, AppError> {
        let trimmed = sub_or_username.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidArgument(
                "User identifier cannot be empty".into(),
            ));
        }

        // Try direct select by sub first
        let by_sub: Option<UserProfile> = self.db.select(("user_profile", trimmed)).await?;
        if let Some(user) = by_sub {
            return Ok(user);
        }

        // Query by username or email
        let lower = trimmed.to_lowercase();
        let mut resp = self
            .db
            .query("SELECT * FROM user_profile WHERE username = $lower OR email = $lower LIMIT 1;")
            .bind(("lower", lower))
            .await?;

        let users: Vec<UserProfile> = resp.take(0)?;
        users
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound(format!("User '{trimmed}' not found")))
    }

    pub async fn list_users(&self, limit: Option<usize>) -> Result<Vec<UserProfile>, AppError> {
        let max = limit.unwrap_or(100);
        let mut resp = self
            .db
            .query("SELECT * FROM user_profile ORDER BY username ASC LIMIT $limit;")
            .bind(("limit", max))
            .await?;

        let users: Vec<UserProfile> = resp.take(0)?;
        Ok(users)
    }

    pub async fn update_user_role(
        &self,
        sub_or_username: &str,
        role: PlatformRole,
        as_override: bool,
    ) -> Result<UserProfile, AppError> {
        let mut user = self.get_user(sub_or_username).await?;
        if as_override {
            user.role_override = Some(role);
        } else {
            user.idp_role = role;
        }
        user.updated_at = Self::now_iso();

        let updated: Option<UserProfile> = self
            .db
            .upsert(("user_profile", user.sub.as_str()))
            .content(user.clone())
            .await?;

        Ok(updated.unwrap_or(user))
    }

    pub async fn update_user(
        &self,
        sub_or_username: &str,
        patch: UpdateUserRequest,
    ) -> Result<UserProfile, AppError> {
        let mut user = self.get_user(sub_or_username).await?;

        if let Some(name) = patch.name {
            user.name = Some(name);
        }
        if let Some(email) = patch.email {
            user.email = Some(email);
        }
        if let Some(namespace) = patch.namespace {
            user.namespace = Some(namespace);
        }
        if let Some(role) = patch.role {
            user.idp_role = role;
        }
        if let Some(override_opt) = patch.role_override {
            user.role_override = override_opt;
        }
        user.updated_at = Self::now_iso();

        let updated: Option<UserProfile> = self
            .db
            .upsert(("user_profile", user.sub.as_str()))
            .content(user.clone())
            .await?;

        Ok(updated.unwrap_or(user))
    }

    pub async fn delete_user(&self, sub_or_username: &str) -> Result<bool, AppError> {
        let user = match self.get_user(sub_or_username).await {
            Ok(u) => u,
            Err(AppError::NotFound(_)) => return Ok(false),
            Err(e) => return Err(e),
        };

        let deleted: Option<UserProfile> =
            self.db.delete(("user_profile", user.sub.as_str())).await?;
        if deleted.is_none() {
            return Ok(false);
        }

        // Clean up group memberships for this user
        let _ = self
            .db
            .query("DELETE FROM group_member WHERE userSub = $sub OR username = $username;")
            .bind(("sub", user.sub.clone()))
            .bind(("username", user.username.clone()))
            .await;

        Ok(true)
    }

    pub async fn get_effective_access(
        &self,
        sub_or_username: &str,
    ) -> Result<EffectiveAccessContext, AppError> {
        let user = self.get_user(sub_or_username).await?;

        let platform_role = user
            .role_override
            .or(user.simulated_role)
            .unwrap_or(user.idp_role);

        let capabilities = platform_role.capabilities();
        let is_admin = platform_role == PlatformRole::Admin;

        // Fetch assigned group grants from group_member
        let mut claims = Vec::new();

        let mut resp = self
            .db
            .query("SELECT * FROM group_member WHERE userSub = $sub OR username = $username;")
            .bind(("sub", user.sub.clone()))
            .bind(("username", user.username.clone()))
            .await?;

        let members: Vec<GroupMember> = resp.take(0)?;
        for m in members {
            let rights_str: Vec<&'static str> = m.rights.iter().map(|r| r.as_str()).collect();
            if rights_str.contains(&"manager") {
                claims.push(format!("{}:manager", m.group));
            } else {
                claims.push(format!("{}:({})", m.group, rights_str.join(",")));
            }
        }

        // Add spaces owned by this user
        let mut owned_resp = self
            .db
            .query("SELECT * FROM group_namespace WHERE owner = $sub;")
            .bind(("sub", user.sub.clone()))
            .await?;

        let owned_groups: Vec<GroupNamespace> = owned_resp.take(0)?;
        for g in owned_groups {
            let claim = format!("{}:manager", g.name);
            if !claims.contains(&claim) {
                claims.push(claim);
            }
        }

        Ok(EffectiveAccessContext {
            sub: user.sub,
            username: user.username,
            email: user.email,
            idp_role: user.idp_role,
            platform_role,
            simulated_role: user.simulated_role,
            capabilities,
            group_grants: claims,
            is_admin,
        })
    }

    // ========================================================================
    // Group / Space Operations
    // ========================================================================

    pub async fn create_group(&self, req: CreateGroupRequest) -> Result<GroupNamespace, AppError> {
        let group_name = Self::normalize_name(&req.name);
        if group_name.is_empty() {
            return Err(AppError::InvalidArgument(
                "Group name cannot be empty".into(),
            ));
        }

        let existing: Option<GroupNamespace> = self
            .db
            .select(("group_namespace", group_name.as_str()))
            .await?;
        if existing.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "Group/Space '{group_name}' already exists"
            )));
        }

        let parent = if group_name.contains('.') {
            let parts: Vec<&str> = group_name.split('.').collect();
            Some(parts[..parts.len() - 1].join("."))
        } else {
            None
        };

        let now = Self::now_iso();
        let record = GroupNamespace {
            name: group_name.clone(),
            parent,
            owner: req.owner.trim().to_string(),
            description: req.description.unwrap_or_default().trim().to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        let created: Option<GroupNamespace> = self
            .db
            .upsert(("group_namespace", group_name.as_str()))
            .content(record.clone())
            .await?;

        // Assign owner manager rights in group_member
        let owner_sub = req.owner.trim();
        let owner_username = match self.get_user(owner_sub).await {
            Ok(u) => u.username,
            Err(_) => owner_sub.to_lowercase(),
        };

        let member_id = Self::member_record_key(&group_name, &owner_username);
        let member_record = GroupMember {
            group: group_name.clone(),
            username: owner_username.clone(),
            display_name: Some(owner_username),
            user_sub: Some(owner_sub.to_string()),
            rights: GroupRight::all_rights(),
            assigned_by: owner_sub.to_string(),
            created_at: now.clone(),
            updated_at: now,
        };

        let _: Option<GroupMember> = self
            .db
            .upsert(("group_member", member_id.as_str()))
            .content(member_record)
            .await?;

        Ok(created.unwrap_or(record))
    }

    pub async fn get_group(&self, group_name: &str) -> Result<GroupNamespace, AppError> {
        let clean = Self::normalize_name(group_name);
        let group: Option<GroupNamespace> =
            self.db.select(("group_namespace", clean.as_str())).await?;
        group.ok_or_else(|| AppError::NotFound(format!("Group '{clean}' not found")))
    }

    pub async fn list_groups(&self) -> Result<Vec<GroupNamespace>, AppError> {
        let mut resp = self
            .db
            .query("SELECT * FROM group_namespace ORDER BY name ASC;")
            .await?;

        let groups: Vec<GroupNamespace> = resp.take(0)?;
        Ok(groups)
    }

    pub async fn delete_group(&self, group_name: &str) -> Result<bool, AppError> {
        let clean = Self::normalize_name(group_name);
        let deleted: Option<GroupNamespace> =
            self.db.delete(("group_namespace", clean.as_str())).await?;
        if deleted.is_none() {
            return Ok(false);
        }

        // Clean up members for this group
        let _ = self
            .db
            .query("DELETE FROM group_member WHERE group = $group;")
            .bind(("group", clean))
            .await;

        Ok(true)
    }

    pub async fn assign_group_role(
        &self,
        group_name: &str,
        req: AssignGroupMemberRequest,
    ) -> Result<GroupMember, AppError> {
        let clean_group = Self::normalize_name(group_name);
        let clean_user = Self::normalize_name(&req.username);

        if clean_group.is_empty() || clean_user.is_empty() {
            return Err(AppError::InvalidArgument(
                "Group name and username cannot be empty".into(),
            ));
        }

        // Ensure group exists or create namespace entry if missing
        if self.get_group(&clean_group).await.is_err() {
            let now = Self::now_iso();
            let segments: Vec<&str> = clean_group.split('.').collect();
            let parent = if segments.len() > 1 {
                Some(segments[..segments.len() - 1].join("."))
            } else {
                None
            };
            let _: Option<GroupNamespace> = self
                .db
                .upsert(("group_namespace", clean_group.as_str()))
                .content(GroupNamespace {
                    name: clean_group.clone(),
                    parent,
                    owner: req.assigned_by.clone().unwrap_or_else(|| "system".into()),
                    description: "".into(),
                    created_at: now.clone(),
                    updated_at: now,
                })
                .await?;
        }

        // Resolve user_sub and display name if user is in user_profile
        let (resolved_sub, display_name) = match self.get_user(&clean_user).await {
            Ok(u) => (Some(u.sub), Some(u.name.unwrap_or(u.username))),
            Err(_) => (None, Some(clean_user.clone())),
        };

        let mut rights = req.rights;
        if rights.is_empty() {
            rights = vec![GroupRight::View, GroupRight::Use];
        } else if rights.contains(&GroupRight::Manager) {
            rights = GroupRight::all_rights();
        }

        let now = Self::now_iso();
        let member_key = Self::member_record_key(&clean_group, &clean_user);
        let member = GroupMember {
            group: clean_group,
            username: clean_user,
            display_name,
            user_sub: resolved_sub,
            rights,
            assigned_by: req.assigned_by.unwrap_or_else(|| "admin".into()),
            created_at: now.clone(),
            updated_at: now,
        };

        let upserted: Option<GroupMember> = self
            .db
            .upsert(("group_member", member_key.as_str()))
            .content(member.clone())
            .await?;

        Ok(upserted.unwrap_or(member))
    }

    pub async fn remove_group_member(
        &self,
        group_name: &str,
        username: &str,
    ) -> Result<bool, AppError> {
        let clean_group = Self::normalize_name(group_name);
        let clean_user = Self::normalize_name(username);
        let member_key = Self::member_record_key(&clean_group, &clean_user);

        let deleted: Option<GroupMember> = self
            .db
            .delete(("group_member", member_key.as_str()))
            .await?;
        if deleted.is_some() {
            return Ok(true);
        }

        // Try deleting by field matching if key did not match
        let mut resp = self
            .db
            .query("DELETE FROM group_member WHERE group = $group AND (username = $user OR userSub = $user);")
            .bind(("group", clean_group))
            .bind(("user", clean_user))
            .await?;

        let rows: Vec<GroupMember> = resp.take(0)?;
        Ok(!rows.is_empty())
    }

    pub async fn list_group_members(&self, group_name: &str) -> Result<Vec<GroupMember>, AppError> {
        let clean = Self::normalize_name(group_name);
        let mut resp = self
            .db
            .query("SELECT * FROM group_member WHERE group = $group ORDER BY username ASC;")
            .bind(("group", clean))
            .await?;

        let members: Vec<GroupMember> = resp.take(0)?;
        Ok(members)
    }

    pub async fn list_user_groups(
        &self,
        sub_or_username: &str,
    ) -> Result<Vec<GroupMember>, AppError> {
        let clean = Self::normalize_name(sub_or_username);
        let mut resp = self
            .db
            .query("SELECT * FROM group_member WHERE username = $user OR userSub = $user ORDER BY group ASC;")
            .bind(("user", clean))
            .await?;

        let members: Vec<GroupMember> = resp.take(0)?;
        Ok(members)
    }

    // ========================================================================
    // SurrealDB Database-level Users
    // ========================================================================

    pub async fn provision_db_user(
        &self,
        req: ProvisionDbUserRequest,
    ) -> Result<DbUserRecord, AppError> {
        let username = req.username.trim();
        if username.is_empty() {
            return Err(AppError::InvalidArgument(
                "DB Username cannot be empty".into(),
            ));
        }
        if req.password.trim().is_empty() {
            return Err(AppError::InvalidArgument(
                "DB Password cannot be empty".into(),
            ));
        }

        let roles = req.roles.unwrap_or_else(|| vec!["OWNER".to_string()]);
        let roles_sql = roles.join(", ");

        let sql =
            format!("DEFINE USER {username} ON DATABASE PASSWORD $password ROLES {roles_sql};");

        self.db
            .query(sql)
            .bind(("password", req.password))
            .await?
            .check()?;

        Ok(DbUserRecord {
            username: username.to_string(),
            roles,
            level: "DATABASE".to_string(),
        })
    }

    pub async fn list_db_users(&self) -> Result<Vec<DbUserRecord>, AppError> {
        let mut resp = self.db.query("INFO FOR DB;").await?;
        let info_val: Option<serde_json::Value> = resp.take(0)?;

        let mut results = Vec::new();
        if let Some(info) = info_val
            && let Some(users_map) = info.get("users").and_then(|u| u.as_object())
        {
            for (name, def) in users_map {
                let def_str = def.as_str().unwrap_or_default();
                let mut roles = Vec::new();
                if def_str.contains("OWNER") {
                    roles.push("OWNER".to_string());
                }
                if def_str.contains("EDITOR") {
                    roles.push("EDITOR".to_string());
                }
                if def_str.contains("VIEWER") {
                    roles.push("VIEWER".to_string());
                }
                if roles.is_empty() {
                    roles.push("CUSTOM".to_string());
                }

                results.push(DbUserRecord {
                    username: name.clone(),
                    roles,
                    level: "DATABASE".to_string(),
                });
            }
        }

        Ok(results)
    }

    pub async fn remove_db_user(&self, username: &str) -> Result<bool, AppError> {
        let clean = username.trim();
        if clean.is_empty() {
            return Err(AppError::InvalidArgument(
                "DB Username cannot be empty".into(),
            ));
        }

        let sql = format!("REMOVE USER {clean} ON DATABASE;");
        self.db.query(sql).await?.check()?;
        Ok(true)
    }
}
