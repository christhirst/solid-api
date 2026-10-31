use chrono::Utc;
use surrealdb::Surreal;
use surrealdb::engine::any::{Any, connect};
use surrealdb::opt::auth::{Database, Root};
use uuid::Uuid;

use crate::config::SurrealDbConfig;
use crate::error::AppError;
use crate::models::{
    AssignGroupMemberRequest, CaCertRecord, ConnectionRecord, CreateConnectionInput,
    CreateDashboardInput, CreateDatabaseInput, CreateGroupRequest, CreateWorkflowInput,
    DashboardRecord, DatabaseMetaRecord, DbUserRecord, EffectiveAccessContext, GroupMember,
    GroupNamespace, GroupRight, PlatformRole, ProtoFileRecord, ProvisionDbUserRequest,
    ProvisionUserRequest, UpdateCaCertInput, UpdateConnectionInput, UpdateDashboardInput,
    UpdateDatabaseMetaInput, UpdateProtoInput, UpdateUserRequest, UpdateWorkflowInput,
    UploadCaCertInput, UploadProtoInput, UserProfile, WorkflowRecord,
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

    fn normalize_tags(tags: Vec<String>) -> Vec<String> {
        let mut out: Vec<String> = tags
            .into_iter()
            .map(|t| Self::normalize_name(&t))
            .filter(|t| !t.is_empty())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    fn extract_key(id_or_key: &str, table: &str) -> String {
        let trimmed = id_or_key
            .trim()
            .trim_matches(|c| c == '⟨' || c == '⟩' || c == '`');
        let prefix = format!("{table}:");
        let raw_key = if let Some(stripped) = trimmed.strip_prefix(&prefix) {
            stripped
        } else {
            trimmed
        };
        raw_key
            .trim_matches(|c| c == '⟨' || c == '⟩' || c == '`')
            .to_string()
    }

    fn canonical_id(table: &str, key: &str) -> String {
        format!("{table}:{key}")
    }

    fn parse_json_or(
        raw: Option<String>,
        fallback: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        match raw {
            Some(s) if !s.trim().is_empty() => {
                let parsed: serde_json::Value = serde_json::from_str(&s)?;
                Ok(parsed)
            }
            _ => Ok(fallback),
        }
    }

    fn member_record_key(group: &str, username: &str) -> String {
        format!(
            "{}__{}",
            Self::normalize_name(group),
            Self::normalize_name(username)
        )
    }

    // ========================================================================
    // 1. User Operations
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
                .map(|ns| Self::normalize_name(&ns))
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
        let trimmed = Self::extract_key(sub_or_username, "user_profile");
        if trimmed.is_empty() {
            return Err(AppError::InvalidArgument(
                "User identifier cannot be empty".into(),
            ));
        }

        let by_sub: Option<UserProfile> =
            self.db.select(("user_profile", trimmed.as_str())).await?;
        if let Some(user) = by_sub {
            return Ok(user);
        }

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
            let clean_ns = Self::normalize_name(&namespace);
            user.namespace = if clean_ns.is_empty() {
                None
            } else {
                Some(clean_ns)
            };
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
    // 2. Group / Space Operations
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
    // 3. SurrealDB Database-level Users
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

    // ========================================================================
    // 4. Dashboards
    // ========================================================================

    pub async fn create_dashboard(
        &self,
        req: CreateDashboardInput,
    ) -> Result<DashboardRecord, AppError> {
        let name = req.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::InvalidArgument(
                "Dashboard name cannot be empty".into(),
            ));
        }

        let key = req
            .id
            .map(|id| Self::extract_key(&id, "dashboard"))
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let owner = req
            .owner
            .map(|o| o.trim().to_string())
            .filter(|o| !o.is_empty())
            .unwrap_or_else(|| "system".to_string());

        // Resolve owner's personal namespace if set
        let owner_ns = match self.get_user(&owner).await {
            Ok(u) => u.namespace,
            Err(_) => None,
        };

        let slug = req
            .slug
            .map(|s| Self::normalize_name(&s))
            .filter(|s| !s.is_empty());

        let buttons = Self::parse_json_or(req.buttons_json, serde_json::Value::Array(Vec::new()))?;
        if !buttons.is_array() {
            return Err(AppError::InvalidArgument(
                "Dashboard buttons_json must be a JSON array".into(),
            ));
        }

        let now = Self::now_iso();
        let record_id = Self::canonical_id("dashboard", &key);
        let record = DashboardRecord {
            record_id: record_id.clone(),
            name,
            description: req.description.unwrap_or_default().trim().to_string(),
            owner,
            namespace: owner_ns,
            slug,
            is_public: req.is_public,
            tags: Self::normalize_tags(req.tags),
            buttons,
            created_at: now.clone(),
            updated_at: now,
        };

        let created: Option<DashboardRecord> = self
            .db
            .upsert(("dashboard", key.as_str()))
            .content(record.clone())
            .await?;

        let mut out = created.unwrap_or(record);
        if out.record_id.is_empty() {
            out.record_id = record_id;
        }
        Ok(out)
    }

    pub async fn get_dashboard(&self, id: &str) -> Result<DashboardRecord, AppError> {
        let key = Self::extract_key(id, "dashboard");
        if key.is_empty() {
            return Err(AppError::InvalidArgument(
                "Dashboard ID cannot be empty".into(),
            ));
        }

        let record: Option<DashboardRecord> = self.db.select(("dashboard", key.as_str())).await?;
        let mut out =
            record.ok_or_else(|| AppError::NotFound(format!("Dashboard '{id}' not found")))?;
        if out.record_id.is_empty() {
            out.record_id = Self::canonical_id("dashboard", &key);
        }
        Ok(out)
    }

    pub async fn list_dashboards(
        &self,
        query: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<DashboardRecord>, AppError> {
        let mut resp = self
            .db
            .query("SELECT *, <string>id AS record_id FROM dashboard ORDER BY updated_at DESC;")
            .await?;

        let all: Vec<DashboardRecord> = resp.take(0)?;
        let q = query.map(|s| s.trim().to_lowercase()).unwrap_or_default();

        let filtered = all
            .into_iter()
            .filter(|d| {
                if published_only && !d.is_public {
                    return false;
                }
                if !q.is_empty() {
                    let match_name = d.name.to_lowercase().contains(&q);
                    let match_desc = d.description.to_lowercase().contains(&q);
                    let match_slug = d
                        .slug
                        .as_deref()
                        .map(|s| s.to_lowercase().contains(&q))
                        .unwrap_or(false);
                    return match_name || match_desc || match_slug;
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    pub async fn update_dashboard(
        &self,
        id: &str,
        patch: UpdateDashboardInput,
    ) -> Result<DashboardRecord, AppError> {
        let key = Self::extract_key(id, "dashboard");
        let mut record = self.get_dashboard(&key).await?;

        if let Some(name) = patch.name {
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() {
                record.name = trimmed;
            }
        }
        if let Some(desc) = patch.description {
            record.description = desc;
        }
        if let Some(owner) = patch.owner {
            record.owner = owner;
        }
        if let Some(slug) = patch.slug {
            let clean = Self::normalize_name(&slug);
            record.slug = if clean.is_empty() { None } else { Some(clean) };
        }
        if let Some(is_public) = patch.is_public {
            record.is_public = is_public;
        }
        if let Some(tags) = patch.tags {
            record.tags = Self::normalize_tags(tags);
        }
        if let Some(buttons_json) = patch.buttons_json {
            let parsed =
                Self::parse_json_or(Some(buttons_json), serde_json::Value::Array(Vec::new()))?;
            if !parsed.is_array() {
                return Err(AppError::InvalidArgument(
                    "Dashboard buttons_json must be a JSON array".into(),
                ));
            }
            record.buttons = parsed;
        }

        // Sync owner namespace if available
        if let Ok(u) = self.get_user(&record.owner).await
            && u.namespace.is_some()
        {
            record.namespace = u.namespace;
        }

        record.updated_at = Self::now_iso();
        record.record_id = Self::canonical_id("dashboard", &key);

        let updated: Option<DashboardRecord> = self
            .db
            .upsert(("dashboard", key.as_str()))
            .content(record.clone())
            .await?;

        Ok(updated.unwrap_or(record))
    }

    pub async fn delete_dashboard(&self, id: &str) -> Result<bool, AppError> {
        let key = Self::extract_key(id, "dashboard");
        let deleted: Option<DashboardRecord> = self.db.delete(("dashboard", key.as_str())).await?;
        Ok(deleted.is_some())
    }

    pub async fn save_user_namespace(
        &self,
        sub_or_username: &str,
        namespace: &str,
    ) -> Result<(String, String), AppError> {
        let clean_ns = Self::normalize_name(namespace);
        if clean_ns.is_empty() {
            return Err(AppError::InvalidArgument(
                "Namespace cannot be empty".into(),
            ));
        }

        let mut user = self.get_user(sub_or_username).await?;

        // Check if another user owns this namespace
        let mut resp = self
            .db
            .query("SELECT * FROM user_profile WHERE namespace = $ns LIMIT 1;")
            .bind(("ns", clean_ns.clone()))
            .await?;
        let existing: Vec<UserProfile> = resp.take(0)?;
        if let Some(other) = existing.first()
            && other.sub != user.sub
        {
            return Err(AppError::AlreadyExists(format!(
                "Namespace '{clean_ns}' is already taken"
            )));
        }

        user.namespace = Some(clean_ns.clone());
        user.updated_at = Self::now_iso();

        let _: Option<UserProfile> = self
            .db
            .upsert(("user_profile", user.sub.as_str()))
            .content(user.clone())
            .await?;

        // Cascade namespace update to dashboards owned by this user
        let _ = self
            .db
            .query("UPDATE dashboard SET namespace = $ns WHERE owner = $owner;")
            .bind(("ns", clean_ns.clone()))
            .bind(("owner", user.sub.clone()))
            .await;

        Ok((user.sub, clean_ns))
    }

    // ========================================================================
    // 5. Workflows
    // ========================================================================

    pub async fn create_workflow(
        &self,
        req: CreateWorkflowInput,
    ) -> Result<WorkflowRecord, AppError> {
        let name = req.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::InvalidArgument(
                "Workflow name cannot be empty".into(),
            ));
        }

        let key = req
            .id
            .map(|id| Self::extract_key(&id, "workflow"))
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let steps = Self::parse_json_or(req.steps_json, serde_json::Value::Array(Vec::new()))?;
        if !steps.is_array() {
            return Err(AppError::InvalidArgument(
                "Workflow steps_json must be a JSON array".into(),
            ));
        }

        let graph = match req.graph_json {
            Some(g) if !g.trim().is_empty() => Some(serde_json::from_str::<serde_json::Value>(&g)?),
            _ => None,
        };

        let now = Self::now_iso();
        let record_id = Self::canonical_id("workflow", &key);
        let record = WorkflowRecord {
            record_id: record_id.clone(),
            name,
            description: req.description.unwrap_or_default().trim().to_string(),
            owner: req
                .owner
                .filter(|o| !o.trim().is_empty())
                .unwrap_or_else(|| "system".to_string()),
            schedule: req.schedule.filter(|s| !s.trim().is_empty()),
            server_address: req.server_address.filter(|s| !s.trim().is_empty()),
            use_tls: req.use_tls,
            direction: req
                .direction
                .filter(|d| !d.trim().is_empty())
                .unwrap_or_else(|| "td".to_string()),
            visibility: req
                .visibility
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "private".to_string()),
            tags: Self::normalize_tags(req.tags),
            steps,
            graph,
            created_at: now.clone(),
            updated_at: now,
        };

        let created: Option<WorkflowRecord> = self
            .db
            .upsert(("workflow", key.as_str()))
            .content(record.clone())
            .await?;

        let mut out = created.unwrap_or(record);
        if out.record_id.is_empty() {
            out.record_id = record_id;
        }
        Ok(out)
    }

    pub async fn get_workflow(&self, id: &str) -> Result<WorkflowRecord, AppError> {
        let key = Self::extract_key(id, "workflow");
        if key.is_empty() {
            return Err(AppError::InvalidArgument(
                "Workflow ID cannot be empty".into(),
            ));
        }

        let record: Option<WorkflowRecord> = self.db.select(("workflow", key.as_str())).await?;
        let mut out =
            record.ok_or_else(|| AppError::NotFound(format!("Workflow '{id}' not found")))?;
        if out.record_id.is_empty() {
            out.record_id = Self::canonical_id("workflow", &key);
        }
        Ok(out)
    }

    pub async fn list_workflows(
        &self,
        query: Option<&str>,
    ) -> Result<Vec<WorkflowRecord>, AppError> {
        let mut resp = self
            .db
            .query("SELECT *, <string>id AS record_id FROM workflow ORDER BY updated_at DESC;")
            .await?;

        let all: Vec<WorkflowRecord> = resp.take(0)?;
        let q = query.map(|s| s.trim().to_lowercase()).unwrap_or_default();

        let filtered = all
            .into_iter()
            .filter(|w| {
                if !q.is_empty() {
                    return w.name.to_lowercase().contains(&q)
                        || w.description.to_lowercase().contains(&q);
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    pub async fn update_workflow(
        &self,
        id: &str,
        patch: UpdateWorkflowInput,
    ) -> Result<WorkflowRecord, AppError> {
        let key = Self::extract_key(id, "workflow");
        let mut record = self.get_workflow(&key).await?;

        if let Some(name) = patch.name {
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() {
                record.name = trimmed;
            }
        }
        if let Some(desc) = patch.description {
            record.description = desc;
        }
        if let Some(owner) = patch.owner {
            record.owner = owner;
        }
        if let Some(schedule) = patch.schedule {
            record.schedule = if schedule.trim().is_empty() {
                None
            } else {
                Some(schedule)
            };
        }
        if let Some(addr) = patch.server_address {
            record.server_address = if addr.trim().is_empty() {
                None
            } else {
                Some(addr)
            };
        }
        if let Some(use_tls) = patch.use_tls {
            record.use_tls = use_tls;
        }
        if let Some(dir) = patch.direction {
            record.direction = dir;
        }
        if let Some(vis) = patch.visibility {
            record.visibility = vis;
        }
        if let Some(tags) = patch.tags {
            record.tags = Self::normalize_tags(tags);
        }
        if let Some(steps_json) = patch.steps_json {
            let parsed =
                Self::parse_json_or(Some(steps_json), serde_json::Value::Array(Vec::new()))?;
            if !parsed.is_array() {
                return Err(AppError::InvalidArgument(
                    "Workflow steps_json must be a JSON array".into(),
                ));
            }
            record.steps = parsed;
        }
        if let Some(graph_json) = patch.graph_json {
            if graph_json.trim().is_empty() {
                record.graph = None;
            } else {
                record.graph = Some(serde_json::from_str::<serde_json::Value>(&graph_json)?);
            }
        }

        record.updated_at = Self::now_iso();
        record.record_id = Self::canonical_id("workflow", &key);

        let updated: Option<WorkflowRecord> = self
            .db
            .upsert(("workflow", key.as_str()))
            .content(record.clone())
            .await?;

        Ok(updated.unwrap_or(record))
    }

    pub async fn delete_workflow(&self, id: &str) -> Result<bool, AppError> {
        let key = Self::extract_key(id, "workflow");
        let deleted: Option<WorkflowRecord> = self.db.delete(("workflow", key.as_str())).await?;
        Ok(deleted.is_some())
    }

    pub async fn clone_workflow(
        &self,
        source_id: &str,
        new_name: Option<String>,
        owner: Option<String>,
    ) -> Result<WorkflowRecord, AppError> {
        let src = self.get_workflow(source_id).await?;
        let key = Uuid::new_v4().to_string();
        let record_id = Self::canonical_id("workflow", &key);
        let now = Self::now_iso();

        let clone_name = new_name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| format!("{} (Copy)", src.name));

        let clone_owner = owner.filter(|o| !o.trim().is_empty()).unwrap_or(src.owner);

        let record = WorkflowRecord {
            record_id: record_id.clone(),
            name: clone_name,
            description: src.description,
            owner: clone_owner,
            schedule: None, // Cloned workflows start with schedule disabled for safety
            server_address: src.server_address,
            use_tls: src.use_tls,
            direction: src.direction,
            visibility: "private".to_string(),
            tags: src.tags,
            steps: src.steps,
            graph: src.graph,
            created_at: now.clone(),
            updated_at: now,
        };

        let created: Option<WorkflowRecord> = self
            .db
            .upsert(("workflow", key.as_str()))
            .content(record.clone())
            .await?;

        Ok(created.unwrap_or(record))
    }

    // ========================================================================
    // 6. Connections
    // ========================================================================

    pub async fn create_connection(
        &self,
        req: CreateConnectionInput,
    ) -> Result<ConnectionRecord, AppError> {
        let name = req.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::InvalidArgument(
                "Connection name cannot be empty".into(),
            ));
        }

        let conn_type = req.conn_type.trim().to_lowercase();
        if conn_type.is_empty() {
            return Err(AppError::InvalidArgument(
                "Connection type cannot be empty".into(),
            ));
        }

        let key = req
            .id
            .map(|id| Self::extract_key(&id, "connection"))
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let config = Self::parse_json_or(
            req.config_json,
            serde_json::Value::Object(serde_json::Map::new()),
        )?;

        let now = Self::now_iso();
        let record_id = Self::canonical_id("connection", &key);
        let record = ConnectionRecord {
            record_id: record_id.clone(),
            name,
            conn_type,
            description: req.description.unwrap_or_default().trim().to_string(),
            owner: req
                .owner
                .filter(|o| !o.trim().is_empty())
                .unwrap_or_else(|| "system".to_string()),
            visibility: req
                .visibility
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "private".to_string()),
            tags: Self::normalize_tags(req.tags),
            config: config.clone(),
            created_at: now.clone(),
            updated_at: now,
        };

        let created: Option<ConnectionRecord> = self
            .db
            .upsert(("connection", key.as_str()))
            .content(record.clone())
            .await?;

        // Also merge top-level config properties onto the SurrealDB record so
        // solid.grpcfront reads `url`, `serverAddress`, `database`, `namespace`, etc. directly.
        if config.is_object() {
            let _: Option<serde_json::Value> = self
                .db
                .update(("connection", key.as_str()))
                .merge(config)
                .await?;
        }

        let mut out = created.unwrap_or(record);
        if out.record_id.is_empty() {
            out.record_id = record_id;
        }
        Ok(out)
    }

    pub async fn get_connection(&self, id: &str) -> Result<ConnectionRecord, AppError> {
        let key = Self::extract_key(id, "connection");
        if key.is_empty() {
            return Err(AppError::InvalidArgument(
                "Connection ID cannot be empty".into(),
            ));
        }

        let record: Option<ConnectionRecord> = self.db.select(("connection", key.as_str())).await?;
        let mut out =
            record.ok_or_else(|| AppError::NotFound(format!("Connection '{id}' not found")))?;
        if out.record_id.is_empty() {
            out.record_id = Self::canonical_id("connection", &key);
        }
        Ok(out)
    }

    pub async fn list_connections(
        &self,
        conn_type: Option<&str>,
    ) -> Result<Vec<ConnectionRecord>, AppError> {
        let mut resp = self
            .db
            .query("SELECT *, <string>id AS record_id FROM connection ORDER BY name ASC;")
            .await?;

        let all: Vec<ConnectionRecord> = resp.take(0)?;
        let filter_type = conn_type
            .map(|t| t.trim().to_lowercase())
            .unwrap_or_default();

        let filtered = all
            .into_iter()
            .filter(|c| {
                if !filter_type.is_empty() {
                    return c.conn_type.to_lowercase() == filter_type;
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    pub async fn update_connection(
        &self,
        id: &str,
        patch: UpdateConnectionInput,
    ) -> Result<ConnectionRecord, AppError> {
        let key = Self::extract_key(id, "connection");
        let mut record = self.get_connection(&key).await?;

        if let Some(name) = patch.name {
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() {
                record.name = trimmed;
            }
        }
        if let Some(conn_type) = patch.conn_type {
            let trimmed = conn_type.trim().to_lowercase();
            if !trimmed.is_empty() {
                record.conn_type = trimmed;
            }
        }
        if let Some(desc) = patch.description {
            record.description = desc;
        }
        if let Some(owner) = patch.owner {
            record.owner = owner;
        }
        if let Some(vis) = patch.visibility {
            record.visibility = vis;
        }
        if let Some(tags) = patch.tags {
            record.tags = Self::normalize_tags(tags);
        }
        if let Some(config_json) = patch.config_json {
            record.config = Self::parse_json_or(
                Some(config_json),
                serde_json::Value::Object(serde_json::Map::new()),
            )?;
        }

        record.updated_at = Self::now_iso();
        record.record_id = Self::canonical_id("connection", &key);

        let updated: Option<ConnectionRecord> = self
            .db
            .upsert(("connection", key.as_str()))
            .content(record.clone())
            .await?;

        if record.config.is_object() {
            let _: Option<serde_json::Value> = self
                .db
                .update(("connection", key.as_str()))
                .merge(record.config.clone())
                .await?;
        }

        Ok(updated.unwrap_or(record))
    }

    pub async fn delete_connection(&self, id: &str) -> Result<bool, AppError> {
        let key = Self::extract_key(id, "connection");
        let deleted: Option<ConnectionRecord> =
            self.db.delete(("connection", key.as_str())).await?;
        Ok(deleted.is_some())
    }

    // ========================================================================
    // 7. Proto Registry & Trusted CA Certificates
    // ========================================================================

    pub async fn upload_proto(&self, req: UploadProtoInput) -> Result<ProtoFileRecord, AppError> {
        let name = req.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::InvalidArgument(
                "Proto file name cannot be empty".into(),
            ));
        }
        if req.content.trim().is_empty() {
            return Err(AppError::InvalidArgument(
                "Proto content cannot be empty".into(),
            ));
        }

        let key = req
            .id
            .map(|id| Self::extract_key(&id, "proto_file"))
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let now = Self::now_iso();
        let record_id = Self::canonical_id("proto_file", &key);
        let record = ProtoFileRecord {
            record_id: record_id.clone(),
            name,
            content: req.content,
            owner: req
                .owner
                .filter(|o| !o.trim().is_empty())
                .unwrap_or_else(|| "system".to_string()),
            visibility: req
                .visibility
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "private".to_string()),
            tags: Self::normalize_tags(req.tags),
            created_at: now.clone(),
            updated_at: now,
        };

        let created: Option<ProtoFileRecord> = self
            .db
            .upsert(("proto_file", key.as_str()))
            .content(record.clone())
            .await?;

        let mut out = created.unwrap_or(record);
        if out.record_id.is_empty() {
            out.record_id = record_id;
        }
        Ok(out)
    }

    pub async fn get_proto(&self, id: &str) -> Result<ProtoFileRecord, AppError> {
        let key = Self::extract_key(id, "proto_file");
        if key.is_empty() {
            return Err(AppError::InvalidArgument("Proto ID cannot be empty".into()));
        }

        let record: Option<ProtoFileRecord> = self.db.select(("proto_file", key.as_str())).await?;
        if let Some(mut out) = record {
            if out.record_id.is_empty() {
                out.record_id = Self::canonical_id("proto_file", &key);
            }
            return Ok(out);
        }

        // Fallback: lookup by name
        let mut resp = self
            .db
            .query("SELECT *, <string>id AS record_id FROM proto_file WHERE name = $name LIMIT 1;")
            .bind(("name", key.clone()))
            .await?;
        let rows: Vec<ProtoFileRecord> = resp.take(0)?;
        rows.into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound(format!("Proto file '{id}' not found")))
    }

    pub async fn list_protos(&self, query: Option<&str>) -> Result<Vec<ProtoFileRecord>, AppError> {
        let mut resp = self
            .db
            .query("SELECT *, <string>id AS record_id FROM proto_file ORDER BY name ASC;")
            .await?;

        let all: Vec<ProtoFileRecord> = resp.take(0)?;
        let q = query.map(|s| s.trim().to_lowercase()).unwrap_or_default();

        let filtered = all
            .into_iter()
            .filter(|p| {
                if !q.is_empty() {
                    return p.name.to_lowercase().contains(&q)
                        || p.content.to_lowercase().contains(&q);
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    pub async fn update_proto(
        &self,
        id: &str,
        patch: UpdateProtoInput,
    ) -> Result<ProtoFileRecord, AppError> {
        let mut record = self.get_proto(id).await?;
        let key = Self::extract_key(&record.record_id, "proto_file");

        if let Some(name) = patch.name {
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() {
                record.name = trimmed;
            }
        }
        if let Some(content) = patch.content {
            record.content = content;
        }
        if let Some(owner) = patch.owner {
            record.owner = owner;
        }
        if let Some(vis) = patch.visibility {
            record.visibility = vis;
        }
        if let Some(tags) = patch.tags {
            record.tags = Self::normalize_tags(tags);
        }

        record.updated_at = Self::now_iso();
        record.record_id = Self::canonical_id("proto_file", &key);

        let updated: Option<ProtoFileRecord> = self
            .db
            .upsert(("proto_file", key.as_str()))
            .content(record.clone())
            .await?;

        Ok(updated.unwrap_or(record))
    }

    pub async fn delete_proto(&self, id: &str) -> Result<bool, AppError> {
        let key = Self::extract_key(id, "proto_file");
        let deleted: Option<ProtoFileRecord> = self.db.delete(("proto_file", key.as_str())).await?;
        Ok(deleted.is_some())
    }

    pub async fn upload_ca_cert(&self, req: UploadCaCertInput) -> Result<CaCertRecord, AppError> {
        let name = req.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::InvalidArgument(
                "CA certificate name cannot be empty".into(),
            ));
        }
        if req.content.trim().is_empty() {
            return Err(AppError::InvalidArgument(
                "CA certificate PEM content cannot be empty".into(),
            ));
        }

        let key = req
            .id
            .map(|id| Self::extract_key(&id, "ca_cert"))
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let now = Self::now_iso();
        let record_id = Self::canonical_id("ca_cert", &key);
        let record = CaCertRecord {
            record_id: record_id.clone(),
            name,
            content: req.content,
            owner: req
                .owner
                .filter(|o| !o.trim().is_empty())
                .unwrap_or_else(|| "system".to_string()),
            visibility: req
                .visibility
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "private".to_string()),
            tags: Self::normalize_tags(req.tags),
            created_at: now.clone(),
            updated_at: now,
        };

        let created: Option<CaCertRecord> = self
            .db
            .upsert(("ca_cert", key.as_str()))
            .content(record.clone())
            .await?;

        let mut out = created.unwrap_or(record);
        if out.record_id.is_empty() {
            out.record_id = record_id;
        }
        Ok(out)
    }

    pub async fn get_ca_cert(&self, id: &str) -> Result<CaCertRecord, AppError> {
        let key = Self::extract_key(id, "ca_cert");
        if key.is_empty() {
            return Err(AppError::InvalidArgument(
                "CA certificate ID cannot be empty".into(),
            ));
        }

        let record: Option<CaCertRecord> = self.db.select(("ca_cert", key.as_str())).await?;
        if let Some(mut out) = record {
            if out.record_id.is_empty() {
                out.record_id = Self::canonical_id("ca_cert", &key);
            }
            return Ok(out);
        }

        let mut resp = self
            .db
            .query("SELECT *, <string>id AS record_id FROM ca_cert WHERE name = $name LIMIT 1;")
            .bind(("name", key.clone()))
            .await?;
        let rows: Vec<CaCertRecord> = resp.take(0)?;
        rows.into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound(format!("CA certificate '{id}' not found")))
    }

    pub async fn list_ca_certs(&self) -> Result<Vec<CaCertRecord>, AppError> {
        let mut resp = self
            .db
            .query("SELECT *, <string>id AS record_id FROM ca_cert ORDER BY name ASC;")
            .await?;

        let certs: Vec<CaCertRecord> = resp.take(0)?;
        Ok(certs)
    }

    pub async fn update_ca_cert(
        &self,
        id: &str,
        patch: UpdateCaCertInput,
    ) -> Result<CaCertRecord, AppError> {
        let mut record = self.get_ca_cert(id).await?;
        let key = Self::extract_key(&record.record_id, "ca_cert");

        if let Some(name) = patch.name {
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() {
                record.name = trimmed;
            }
        }
        if let Some(content) = patch.content {
            record.content = content;
        }
        if let Some(owner) = patch.owner {
            record.owner = owner;
        }
        if let Some(vis) = patch.visibility {
            record.visibility = vis;
        }
        if let Some(tags) = patch.tags {
            record.tags = Self::normalize_tags(tags);
        }

        record.updated_at = Self::now_iso();
        record.record_id = Self::canonical_id("ca_cert", &key);

        let updated: Option<CaCertRecord> = self
            .db
            .upsert(("ca_cert", key.as_str()))
            .content(record.clone())
            .await?;

        Ok(updated.unwrap_or(record))
    }

    pub async fn delete_ca_cert(&self, id: &str) -> Result<bool, AppError> {
        let key = Self::extract_key(id, "ca_cert");
        let deleted: Option<CaCertRecord> = self.db.delete(("ca_cert", key.as_str())).await?;
        Ok(deleted.is_some())
    }

    // ========================================================================
    // 8. Databases & SurrealQL Execution
    // ========================================================================

    pub async fn create_database(
        &self,
        req: CreateDatabaseInput,
    ) -> Result<DatabaseMetaRecord, AppError> {
        let clean_name = req.name.trim().to_string();
        if clean_name.is_empty() {
            return Err(AppError::InvalidArgument(
                "Database name cannot be empty".into(),
            ));
        }

        let sql = format!("DEFINE DATABASE IF NOT EXISTS `{clean_name}`;");
        self.db.query(sql).await?.check()?;

        let now = Self::now_iso();
        let meta = DatabaseMetaRecord {
            name: clean_name.clone(),
            owner: req
                .owner
                .filter(|o| !o.trim().is_empty())
                .unwrap_or_else(|| "system".to_string()),
            tags: Self::normalize_tags(req.tags),
            created_at: now.clone(),
            updated_at: now,
        };

        let created: Option<DatabaseMetaRecord> = self
            .db
            .upsert(("database_meta", clean_name.as_str()))
            .content(meta.clone())
            .await?;

        Ok(created.unwrap_or(meta))
    }

    pub async fn update_database_meta(
        &self,
        name: &str,
        patch: UpdateDatabaseMetaInput,
    ) -> Result<DatabaseMetaRecord, AppError> {
        let clean = Self::extract_key(name, "database_meta");
        if clean.is_empty() {
            return Err(AppError::InvalidArgument(
                "Database name cannot be empty".into(),
            ));
        }

        let existing: Option<DatabaseMetaRecord> =
            self.db.select(("database_meta", clean.as_str())).await?;
        let now = Self::now_iso();
        let mut meta = existing.unwrap_or(DatabaseMetaRecord {
            name: clean.clone(),
            owner: "system".to_string(),
            tags: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
        });

        if let Some(owner) = patch.owner {
            meta.owner = owner;
        }
        if let Some(tags) = patch.tags {
            meta.tags = Self::normalize_tags(tags);
        }
        meta.updated_at = now;

        let updated: Option<DatabaseMetaRecord> = self
            .db
            .upsert(("database_meta", clean.as_str()))
            .content(meta.clone())
            .await?;

        Ok(updated.unwrap_or(meta))
    }

    pub async fn list_databases(&self) -> Result<Vec<DatabaseMetaRecord>, AppError> {
        let mut meta_resp = self
            .db
            .query("SELECT * FROM database_meta ORDER BY name ASC;")
            .await?;
        let mut metas: Vec<DatabaseMetaRecord> = meta_resp.take(0)?;

        // Also inspect INFO FOR NS to include any databases defined in the namespace
        if let Ok(mut ns_resp) = self.db.query("INFO FOR NS;").await
            && let Ok(Some(info)) = ns_resp.take::<Option<serde_json::Value>>(0)
            && let Some(dbs_obj) = info.get("databases").and_then(|d| d.as_object())
        {
            for db_name in dbs_obj.keys() {
                if !metas.iter().any(|m| &m.name == db_name) {
                    metas.push(DatabaseMetaRecord {
                        name: db_name.clone(),
                        owner: "system".to_string(),
                        tags: Vec::new(),
                        created_at: String::new(),
                        updated_at: String::new(),
                    });
                }
            }
        }

        metas.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(metas)
    }

    pub async fn execute_query(
        &self,
        query: &str,
        vars_json: Option<&str>,
    ) -> Result<serde_json::Value, AppError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidArgument("Query cannot be empty".into()));
        }

        let mut q = self.db.query(trimmed);

        if let Some(raw_vars) = vars_json
            && !raw_vars.trim().is_empty()
        {
            let vars_val: serde_json::Value = serde_json::from_str(raw_vars)?;
            if let Some(obj) = vars_val.as_object() {
                for (k, v) in obj {
                    q = q.bind((k.clone(), v.clone()));
                }
            }
        }

        let mut resp = q.await?;
        let num_statements = resp.num_statements();
        let mut results = Vec::with_capacity(num_statements);

        for idx in 0..num_statements {
            let val: surrealdb::types::Value = resp.take(idx)?;
            results.push(serde_json::to_value(val)?);
        }

        if results.len() == 1 {
            Ok(results
                .into_iter()
                .next()
                .unwrap_or(serde_json::Value::Null))
        } else {
            Ok(serde_json::Value::Array(results))
        }
    }
}
