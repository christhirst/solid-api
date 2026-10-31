use tonic::{Request, Response, Status};

use crate::grpc::pb::connection_service_server::ConnectionService;
use crate::grpc::pb::dashboard_service_server::DashboardService;
use crate::grpc::pb::database_service_server::DatabaseService;
use crate::grpc::pb::registry_service_server::RegistryService;
use crate::grpc::pb::user_provisioning_service_server::UserProvisioningService;
use crate::grpc::pb::workflow_service_server::WorkflowService;
use crate::grpc::pb::*;
use crate::models::{
    AssignGroupMemberRequest as ModelAssignGroupMemberRequest, CaCertRecord, ConnectionRecord,
    CreateConnectionInput, CreateDashboardInput, CreateDatabaseInput,
    CreateGroupRequest as ModelCreateGroupRequest, CreateWorkflowInput, DashboardRecord,
    DatabaseMetaRecord, EffectiveAccessContext, GroupMember, GroupNamespace, GroupRight,
    PlatformCapabilities, PlatformRole, ProtoFileRecord,
    ProvisionDbUserRequest as ModelProvisionDbUserRequest,
    ProvisionUserRequest as ModelProvisionUserRequest, UpdateCaCertInput, UpdateConnectionInput,
    UpdateDashboardInput, UpdateDatabaseMetaInput, UpdateProtoInput,
    UpdateUserRequest as ModelUpdateUserRequest, UpdateWorkflowInput, UploadCaCertInput,
    UploadProtoInput, UserProfile, WorkflowRecord,
};
use crate::schema::ensure_schema;
use crate::store::Store;

#[derive(Debug, Clone)]
pub struct GrpcSolidService {
    store: Store,
}

pub type GrpcUserProvisioningService = GrpcSolidService;

impl GrpcSolidService {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

// ============================================================================
// Model -> Protobuf Message Conversions
// ============================================================================

impl From<UserProfile> for UserProfileMsg {
    fn from(u: UserProfile) -> Self {
        Self {
            sub: u.sub,
            username: u.username,
            name: u.name.unwrap_or_default(),
            email: u.email.unwrap_or_default(),
            idp_role: u.idp_role.as_str().to_string(),
            role_override: u
                .role_override
                .map(|r| r.as_str().to_string())
                .unwrap_or_default(),
            simulated_role: u
                .simulated_role
                .map(|r| r.as_str().to_string())
                .unwrap_or_default(),
            namespace: u.namespace.unwrap_or_default(),
            group_grants: u.group_grants,
            disabled_groups: u.disabled_groups,
            disable_owner_bypass: u.disable_owner_bypass,
            created_at: u.created_at.unwrap_or_default(),
            updated_at: u.updated_at,
        }
    }
}

impl From<GroupNamespace> for GroupNamespaceMsg {
    fn from(g: GroupNamespace) -> Self {
        Self {
            name: g.name,
            parent: g.parent.unwrap_or_default(),
            owner: g.owner,
            description: g.description,
            created_at: g.created_at,
            updated_at: g.updated_at,
        }
    }
}

impl From<GroupMember> for GroupMemberMsg {
    fn from(m: GroupMember) -> Self {
        Self {
            group: m.group,
            username: m.username,
            display_name: m.display_name.unwrap_or_default(),
            user_sub: m.user_sub.unwrap_or_default(),
            rights: m.rights.iter().map(|r| r.as_str().to_string()).collect(),
            assigned_by: m.assigned_by,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

impl From<PlatformCapabilities> for PlatformCapabilitiesMsg {
    fn from(c: PlatformCapabilities) -> Self {
        Self {
            can_access_library: c.can_access_library,
            can_edit_dashboards: c.can_edit_dashboards,
            can_manage_workflows: c.can_manage_workflows,
            can_use_requests: c.can_use_requests,
            can_manage_connections: c.can_manage_connections,
            can_manage_cas: c.can_manage_cas,
            can_manage_protos: c.can_manage_protos,
            can_manage_database: c.can_manage_database,
            can_manage_groups: c.can_manage_groups,
        }
    }
}

impl From<EffectiveAccessContext> for EffectiveAccessContextMsg {
    fn from(ctx: EffectiveAccessContext) -> Self {
        Self {
            sub: ctx.sub,
            username: ctx.username,
            email: ctx.email.unwrap_or_default(),
            idp_role: ctx.idp_role.as_str().to_string(),
            platform_role: ctx.platform_role.as_str().to_string(),
            simulated_role: ctx
                .simulated_role
                .map(|r| r.as_str().to_string())
                .unwrap_or_default(),
            capabilities: Some(ctx.capabilities.into()),
            group_grants: ctx.group_grants,
            is_admin: ctx.is_admin,
        }
    }
}

impl From<DashboardRecord> for DashboardMsg {
    fn from(d: DashboardRecord) -> Self {
        Self {
            id: d.record_id,
            name: d.name,
            description: d.description,
            owner: d.owner,
            namespace: d.namespace.unwrap_or_default(),
            slug: d.slug.unwrap_or_default(),
            is_public: d.is_public,
            tags: d.tags,
            buttons_json: serde_json::to_string(&d.buttons).unwrap_or_else(|_| "[]".into()),
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}

impl From<WorkflowRecord> for WorkflowMsg {
    fn from(w: WorkflowRecord) -> Self {
        Self {
            id: w.record_id,
            name: w.name,
            description: w.description,
            owner: w.owner,
            schedule: w.schedule.unwrap_or_default(),
            server_address: w.server_address.unwrap_or_default(),
            use_tls: w.use_tls,
            direction: w.direction,
            visibility: w.visibility,
            tags: w.tags,
            steps_json: serde_json::to_string(&w.steps).unwrap_or_else(|_| "[]".into()),
            graph_json: w
                .graph
                .map(|g| serde_json::to_string(&g).unwrap_or_default())
                .unwrap_or_default(),
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

impl From<ConnectionRecord> for ConnectionMsg {
    fn from(c: ConnectionRecord) -> Self {
        Self {
            id: c.record_id,
            name: c.name,
            conn_type: c.conn_type,
            description: c.description,
            owner: c.owner,
            visibility: c.visibility,
            tags: c.tags,
            config_json: serde_json::to_string(&c.config).unwrap_or_else(|_| "{}".into()),
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

impl From<ProtoFileRecord> for ProtoFileMsg {
    fn from(p: ProtoFileRecord) -> Self {
        Self {
            id: p.record_id,
            name: p.name,
            content: p.content,
            owner: p.owner,
            visibility: p.visibility,
            tags: p.tags,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

impl From<CaCertRecord> for CaCertMsg {
    fn from(c: CaCertRecord) -> Self {
        Self {
            id: c.record_id,
            name: c.name,
            content: c.content,
            owner: c.owner,
            visibility: c.visibility,
            tags: c.tags,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

impl From<DatabaseMetaRecord> for DatabaseMetaMsg {
    fn from(d: DatabaseMetaRecord) -> Self {
        Self {
            name: d.name,
            owner: d.owner,
            tags: d.tags,
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}

// ============================================================================
// 1. UserProvisioningService Implementation
// ============================================================================

#[tonic::async_trait]
impl UserProvisioningService for GrpcSolidService {
    async fn init_schema(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<InitSchemaResponse>, Status> {
        ensure_schema(&self.store.db).await.map_err(Status::from)?;
        Ok(Response::new(InitSchemaResponse {
            success: true,
            message: "SurrealDB schema and unique indexes initialized".to_string(),
        }))
    }

    async fn provision_user(
        &self,
        request: Request<ProvisionUserRequest>,
    ) -> Result<Response<UserProfileMsg>, Status> {
        let req = request.into_inner();
        let role = if !req.role.is_empty() {
            Some(PlatformRole::from_str_loose(&req.role).ok_or_else(|| {
                Status::invalid_argument(format!("Invalid platform role '{}'", req.role))
            })?)
        } else {
            None
        };

        let user = self
            .store
            .provision_user(ModelProvisionUserRequest {
                sub: if req.sub.is_empty() {
                    None
                } else {
                    Some(req.sub)
                },
                username: req.username,
                name: if req.name.is_empty() {
                    None
                } else {
                    Some(req.name)
                },
                email: if req.email.is_empty() {
                    None
                } else {
                    Some(req.email)
                },
                role,
                namespace: if req.namespace.is_empty() {
                    None
                } else {
                    Some(req.namespace)
                },
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(user.into()))
    }

    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<UserProfileMsg>, Status> {
        let req = request.into_inner();
        let user = self
            .store
            .get_user(&req.sub_or_username)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(user.into()))
    }

    async fn list_users(
        &self,
        request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit > 0 {
            Some(req.limit as usize)
        } else {
            None
        };
        let users = self.store.list_users(limit).await.map_err(Status::from)?;

        Ok(Response::new(ListUsersResponse {
            users: users.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update_user(
        &self,
        request: Request<UpdateUserRequest>,
    ) -> Result<Response<UserProfileMsg>, Status> {
        let req = request.into_inner();
        let role = match req.role {
            Some(r) if !r.is_empty() => Some(
                PlatformRole::from_str_loose(&r)
                    .ok_or_else(|| Status::invalid_argument(format!("Invalid role '{r}'")))?,
            ),
            _ => None,
        };
        let role_override = match req.role_override {
            Some(ro) if ro.is_empty() => Some(None),
            Some(ro) => Some(Some(PlatformRole::from_str_loose(&ro).ok_or_else(
                || Status::invalid_argument(format!("Invalid role_override '{ro}'")),
            )?)),
            None => None,
        };

        let updated = self
            .store
            .update_user(
                &req.sub_or_username,
                ModelUpdateUserRequest {
                    name: req.name,
                    email: req.email,
                    namespace: req.namespace,
                    role,
                    role_override,
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(updated.into()))
    }

    async fn assign_platform_role(
        &self,
        request: Request<AssignPlatformRoleRequest>,
    ) -> Result<Response<UserProfileMsg>, Status> {
        let req = request.into_inner();
        let role = PlatformRole::from_str_loose(&req.role).ok_or_else(|| {
            Status::invalid_argument(format!("Invalid platform role '{}'", req.role))
        })?;

        let user = self
            .store
            .update_user_role(&req.sub_or_username, role, req.as_override)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(user.into()))
    }

    async fn delete_user(
        &self,
        request: Request<DeleteUserRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let deleted = self
            .store
            .delete_user(&req.sub_or_username)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: deleted,
            message: if deleted {
                "User deleted".to_string()
            } else {
                "User not found".to_string()
            },
        }))
    }

    async fn get_effective_access(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<EffectiveAccessContextMsg>, Status> {
        let req = request.into_inner();
        let ctx = self
            .store
            .get_effective_access(&req.sub_or_username)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ctx.into()))
    }

    async fn create_group(
        &self,
        request: Request<CreateGroupRequest>,
    ) -> Result<Response<GroupNamespaceMsg>, Status> {
        let req = request.into_inner();
        let group = self
            .store
            .create_group(ModelCreateGroupRequest {
                name: req.name,
                owner: req.owner,
                description: if req.description.is_empty() {
                    None
                } else {
                    Some(req.description)
                },
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(group.into()))
    }

    async fn get_group(
        &self,
        request: Request<GetGroupRequest>,
    ) -> Result<Response<GroupNamespaceMsg>, Status> {
        let req = request.into_inner();
        let group = self
            .store
            .get_group(&req.name)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(group.into()))
    }

    async fn list_groups(
        &self,
        _request: Request<ListGroupsRequest>,
    ) -> Result<Response<ListGroupsResponse>, Status> {
        let groups = self.store.list_groups().await.map_err(Status::from)?;
        Ok(Response::new(ListGroupsResponse {
            groups: groups.into_iter().map(Into::into).collect(),
        }))
    }

    async fn delete_group(
        &self,
        request: Request<DeleteGroupRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let deleted = self
            .store
            .delete_group(&req.name)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: deleted,
            message: if deleted {
                "Group deleted".to_string()
            } else {
                "Group not found".to_string()
            },
        }))
    }

    async fn assign_group_member(
        &self,
        request: Request<AssignGroupMemberRequest>,
    ) -> Result<Response<GroupMemberMsg>, Status> {
        let req = request.into_inner();
        let rights: Vec<GroupRight> = req
            .rights
            .iter()
            .filter_map(|r| GroupRight::from_str_loose(r))
            .collect();

        let member = self
            .store
            .assign_group_role(
                &req.group_name,
                ModelAssignGroupMemberRequest {
                    username: req.username,
                    rights,
                    assigned_by: if req.assigned_by.is_empty() {
                        None
                    } else {
                        Some(req.assigned_by)
                    },
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(member.into()))
    }

    async fn remove_group_member(
        &self,
        request: Request<RemoveGroupMemberRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let removed = self
            .store
            .remove_group_member(&req.group_name, &req.username)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: removed,
            message: if removed {
                "Member removed".to_string()
            } else {
                "Member not found".to_string()
            },
        }))
    }

    async fn list_group_members(
        &self,
        request: Request<ListGroupMembersRequest>,
    ) -> Result<Response<ListGroupMembersResponse>, Status> {
        let req = request.into_inner();
        let members = self
            .store
            .list_group_members(&req.group_name)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ListGroupMembersResponse {
            members: members.into_iter().map(Into::into).collect(),
        }))
    }

    async fn list_user_groups(
        &self,
        request: Request<ListUserGroupsRequest>,
    ) -> Result<Response<ListGroupMembersResponse>, Status> {
        let req = request.into_inner();
        let members = self
            .store
            .list_user_groups(&req.sub_or_username)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ListGroupMembersResponse {
            members: members.into_iter().map(Into::into).collect(),
        }))
    }

    async fn provision_db_user(
        &self,
        request: Request<ProvisionDbUserRequest>,
    ) -> Result<Response<DbUserMsg>, Status> {
        let req = request.into_inner();
        let record = self
            .store
            .provision_db_user(ModelProvisionDbUserRequest {
                username: req.username,
                password: req.password,
                roles: if req.roles.is_empty() {
                    None
                } else {
                    Some(req.roles)
                },
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DbUserMsg {
            username: record.username,
            roles: record.roles,
            level: record.level,
        }))
    }

    async fn list_db_users(
        &self,
        _request: Request<ListDbUsersRequest>,
    ) -> Result<Response<ListDbUsersResponse>, Status> {
        let users = self.store.list_db_users().await.map_err(Status::from)?;
        Ok(Response::new(ListDbUsersResponse {
            users: users
                .into_iter()
                .map(|u| DbUserMsg {
                    username: u.username,
                    roles: u.roles,
                    level: u.level,
                })
                .collect(),
        }))
    }

    async fn remove_db_user(
        &self,
        request: Request<RemoveDbUserRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let removed = self
            .store
            .remove_db_user(&req.username)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: removed,
            message: "Database user removed".to_string(),
        }))
    }
}

// ============================================================================
// 2. DashboardService Implementation
// ============================================================================

#[tonic::async_trait]
impl DashboardService for GrpcSolidService {
    async fn create_dashboard(
        &self,
        request: Request<CreateDashboardRequest>,
    ) -> Result<Response<DashboardMsg>, Status> {
        let req = request.into_inner();
        let dashboard = self
            .store
            .create_dashboard(CreateDashboardInput {
                id: if req.id.is_empty() {
                    None
                } else {
                    Some(req.id)
                },
                name: req.name,
                description: if req.description.is_empty() {
                    None
                } else {
                    Some(req.description)
                },
                owner: if req.owner.is_empty() {
                    None
                } else {
                    Some(req.owner)
                },
                slug: if req.slug.is_empty() {
                    None
                } else {
                    Some(req.slug)
                },
                is_public: req.is_public,
                tags: req.tags,
                buttons_json: if req.buttons_json.is_empty() {
                    None
                } else {
                    Some(req.buttons_json)
                },
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(dashboard.into()))
    }

    async fn get_dashboard(
        &self,
        request: Request<GetDashboardRequest>,
    ) -> Result<Response<DashboardMsg>, Status> {
        let req = request.into_inner();
        let dashboard = self
            .store
            .get_dashboard(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(dashboard.into()))
    }

    async fn list_dashboards(
        &self,
        request: Request<ListDashboardsRequest>,
    ) -> Result<Response<ListDashboardsResponse>, Status> {
        let req = request.into_inner();
        let query = if req.query.is_empty() {
            None
        } else {
            Some(req.query.as_str())
        };
        let dashboards = self
            .store
            .list_dashboards(query, req.published_only)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ListDashboardsResponse {
            dashboards: dashboards.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update_dashboard(
        &self,
        request: Request<UpdateDashboardRequest>,
    ) -> Result<Response<DashboardMsg>, Status> {
        let req = request.into_inner();
        let dashboard = self
            .store
            .update_dashboard(
                &req.id,
                UpdateDashboardInput {
                    name: req.name,
                    description: req.description,
                    owner: req.owner,
                    slug: req.slug,
                    is_public: req.is_public,
                    tags: if req.update_tags || !req.tags.is_empty() {
                        Some(req.tags)
                    } else {
                        None
                    },
                    buttons_json: req.buttons_json,
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(dashboard.into()))
    }

    async fn delete_dashboard(
        &self,
        request: Request<DeleteDashboardRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let deleted = self
            .store
            .delete_dashboard(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: deleted,
            message: if deleted {
                "Dashboard deleted".to_string()
            } else {
                "Dashboard not found".to_string()
            },
        }))
    }

    async fn save_user_namespace(
        &self,
        request: Request<SaveUserNamespaceRequest>,
    ) -> Result<Response<SaveUserNamespaceResponse>, Status> {
        let req = request.into_inner();
        let (sub, namespace) = self
            .store
            .save_user_namespace(&req.sub_or_username, &req.namespace)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(SaveUserNamespaceResponse { sub, namespace }))
    }
}

// ============================================================================
// 3. WorkflowService Implementation
// ============================================================================

#[tonic::async_trait]
impl WorkflowService for GrpcSolidService {
    async fn create_workflow(
        &self,
        request: Request<CreateWorkflowRequest>,
    ) -> Result<Response<WorkflowMsg>, Status> {
        let req = request.into_inner();
        let workflow = self
            .store
            .create_workflow(CreateWorkflowInput {
                id: if req.id.is_empty() {
                    None
                } else {
                    Some(req.id)
                },
                name: req.name,
                description: if req.description.is_empty() {
                    None
                } else {
                    Some(req.description)
                },
                owner: if req.owner.is_empty() {
                    None
                } else {
                    Some(req.owner)
                },
                schedule: if req.schedule.is_empty() {
                    None
                } else {
                    Some(req.schedule)
                },
                server_address: if req.server_address.is_empty() {
                    None
                } else {
                    Some(req.server_address)
                },
                use_tls: req.use_tls,
                direction: if req.direction.is_empty() {
                    None
                } else {
                    Some(req.direction)
                },
                visibility: if req.visibility.is_empty() {
                    None
                } else {
                    Some(req.visibility)
                },
                tags: req.tags,
                steps_json: if req.steps_json.is_empty() {
                    None
                } else {
                    Some(req.steps_json)
                },
                graph_json: if req.graph_json.is_empty() {
                    None
                } else {
                    Some(req.graph_json)
                },
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(workflow.into()))
    }

    async fn get_workflow(
        &self,
        request: Request<GetWorkflowRequest>,
    ) -> Result<Response<WorkflowMsg>, Status> {
        let req = request.into_inner();
        let workflow = self
            .store
            .get_workflow(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(workflow.into()))
    }

    async fn list_workflows(
        &self,
        request: Request<ListWorkflowsRequest>,
    ) -> Result<Response<ListWorkflowsResponse>, Status> {
        let req = request.into_inner();
        let query = if req.query.is_empty() {
            None
        } else {
            Some(req.query.as_str())
        };
        let workflows = self
            .store
            .list_workflows(query)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ListWorkflowsResponse {
            workflows: workflows.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update_workflow(
        &self,
        request: Request<UpdateWorkflowRequest>,
    ) -> Result<Response<WorkflowMsg>, Status> {
        let req = request.into_inner();
        let workflow = self
            .store
            .update_workflow(
                &req.id,
                UpdateWorkflowInput {
                    name: req.name,
                    description: req.description,
                    owner: req.owner,
                    schedule: req.schedule,
                    server_address: req.server_address,
                    use_tls: req.use_tls,
                    direction: req.direction,
                    visibility: req.visibility,
                    tags: if req.update_tags || !req.tags.is_empty() {
                        Some(req.tags)
                    } else {
                        None
                    },
                    steps_json: req.steps_json,
                    graph_json: req.graph_json,
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(workflow.into()))
    }

    async fn delete_workflow(
        &self,
        request: Request<DeleteWorkflowRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let deleted = self
            .store
            .delete_workflow(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: deleted,
            message: if deleted {
                "Workflow deleted".to_string()
            } else {
                "Workflow not found".to_string()
            },
        }))
    }

    async fn clone_workflow(
        &self,
        request: Request<CloneWorkflowRequest>,
    ) -> Result<Response<WorkflowMsg>, Status> {
        let req = request.into_inner();
        let workflow = self
            .store
            .clone_workflow(
                &req.source_id,
                if req.new_name.is_empty() {
                    None
                } else {
                    Some(req.new_name)
                },
                if req.owner.is_empty() {
                    None
                } else {
                    Some(req.owner)
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(workflow.into()))
    }
}

// ============================================================================
// 4. ConnectionService Implementation
// ============================================================================

#[tonic::async_trait]
impl ConnectionService for GrpcSolidService {
    async fn create_connection(
        &self,
        request: Request<CreateConnectionRequest>,
    ) -> Result<Response<ConnectionMsg>, Status> {
        let req = request.into_inner();
        let conn = self
            .store
            .create_connection(CreateConnectionInput {
                id: if req.id.is_empty() {
                    None
                } else {
                    Some(req.id)
                },
                name: req.name,
                conn_type: req.conn_type,
                description: if req.description.is_empty() {
                    None
                } else {
                    Some(req.description)
                },
                owner: if req.owner.is_empty() {
                    None
                } else {
                    Some(req.owner)
                },
                visibility: if req.visibility.is_empty() {
                    None
                } else {
                    Some(req.visibility)
                },
                tags: req.tags,
                config_json: if req.config_json.is_empty() {
                    None
                } else {
                    Some(req.config_json)
                },
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(conn.into()))
    }

    async fn get_connection(
        &self,
        request: Request<GetConnectionRequest>,
    ) -> Result<Response<ConnectionMsg>, Status> {
        let req = request.into_inner();
        let conn = self
            .store
            .get_connection(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(conn.into()))
    }

    async fn list_connections(
        &self,
        request: Request<ListConnectionsRequest>,
    ) -> Result<Response<ListConnectionsResponse>, Status> {
        let req = request.into_inner();
        let conn_type = if req.conn_type.is_empty() {
            None
        } else {
            Some(req.conn_type.as_str())
        };
        let connections = self
            .store
            .list_connections(conn_type)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ListConnectionsResponse {
            connections: connections.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update_connection(
        &self,
        request: Request<UpdateConnectionRequest>,
    ) -> Result<Response<ConnectionMsg>, Status> {
        let req = request.into_inner();
        let conn = self
            .store
            .update_connection(
                &req.id,
                UpdateConnectionInput {
                    name: req.name,
                    conn_type: req.conn_type,
                    description: req.description,
                    owner: req.owner,
                    visibility: req.visibility,
                    tags: if req.update_tags || !req.tags.is_empty() {
                        Some(req.tags)
                    } else {
                        None
                    },
                    config_json: req.config_json,
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(conn.into()))
    }

    async fn delete_connection(
        &self,
        request: Request<DeleteConnectionRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let deleted = self
            .store
            .delete_connection(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: deleted,
            message: if deleted {
                "Connection deleted".to_string()
            } else {
                "Connection not found".to_string()
            },
        }))
    }
}

// ============================================================================
// 5. RegistryService Implementation
// ============================================================================

#[tonic::async_trait]
impl RegistryService for GrpcSolidService {
    async fn upload_proto(
        &self,
        request: Request<UploadProtoRequest>,
    ) -> Result<Response<ProtoFileMsg>, Status> {
        let req = request.into_inner();
        let proto = self
            .store
            .upload_proto(UploadProtoInput {
                id: if req.id.is_empty() {
                    None
                } else {
                    Some(req.id)
                },
                name: req.name,
                content: req.content,
                owner: if req.owner.is_empty() {
                    None
                } else {
                    Some(req.owner)
                },
                visibility: if req.visibility.is_empty() {
                    None
                } else {
                    Some(req.visibility)
                },
                tags: req.tags,
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(proto.into()))
    }

    async fn get_proto(
        &self,
        request: Request<GetProtoRequest>,
    ) -> Result<Response<ProtoFileMsg>, Status> {
        let req = request.into_inner();
        let proto = self.store.get_proto(&req.id).await.map_err(Status::from)?;

        Ok(Response::new(proto.into()))
    }

    async fn list_protos(
        &self,
        request: Request<ListProtosRequest>,
    ) -> Result<Response<ListProtosResponse>, Status> {
        let req = request.into_inner();
        let query = if req.query.is_empty() {
            None
        } else {
            Some(req.query.as_str())
        };
        let protos = self.store.list_protos(query).await.map_err(Status::from)?;

        Ok(Response::new(ListProtosResponse {
            protos: protos.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update_proto(
        &self,
        request: Request<UpdateProtoRequest>,
    ) -> Result<Response<ProtoFileMsg>, Status> {
        let req = request.into_inner();
        let proto = self
            .store
            .update_proto(
                &req.id,
                UpdateProtoInput {
                    name: req.name,
                    content: req.content,
                    owner: req.owner,
                    visibility: req.visibility,
                    tags: if req.update_tags || !req.tags.is_empty() {
                        Some(req.tags)
                    } else {
                        None
                    },
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(proto.into()))
    }

    async fn delete_proto(
        &self,
        request: Request<DeleteProtoRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let deleted = self
            .store
            .delete_proto(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: deleted,
            message: if deleted {
                "Proto file deleted".to_string()
            } else {
                "Proto file not found".to_string()
            },
        }))
    }

    async fn upload_ca_cert(
        &self,
        request: Request<UploadCaCertRequest>,
    ) -> Result<Response<CaCertMsg>, Status> {
        let req = request.into_inner();
        let cert = self
            .store
            .upload_ca_cert(UploadCaCertInput {
                id: if req.id.is_empty() {
                    None
                } else {
                    Some(req.id)
                },
                name: req.name,
                content: req.content,
                owner: if req.owner.is_empty() {
                    None
                } else {
                    Some(req.owner)
                },
                visibility: if req.visibility.is_empty() {
                    None
                } else {
                    Some(req.visibility)
                },
                tags: req.tags,
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(cert.into()))
    }

    async fn get_ca_cert(
        &self,
        request: Request<GetCaCertRequest>,
    ) -> Result<Response<CaCertMsg>, Status> {
        let req = request.into_inner();
        let cert = self
            .store
            .get_ca_cert(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(cert.into()))
    }

    async fn list_ca_certs(
        &self,
        _request: Request<ListCaCertsRequest>,
    ) -> Result<Response<ListCaCertsResponse>, Status> {
        let certs = self.store.list_ca_certs().await.map_err(Status::from)?;
        Ok(Response::new(ListCaCertsResponse {
            certs: certs.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update_ca_cert(
        &self,
        request: Request<UpdateCaCertRequest>,
    ) -> Result<Response<CaCertMsg>, Status> {
        let req = request.into_inner();
        let cert = self
            .store
            .update_ca_cert(
                &req.id,
                UpdateCaCertInput {
                    name: req.name,
                    content: req.content,
                    owner: req.owner,
                    visibility: req.visibility,
                    tags: if req.update_tags || !req.tags.is_empty() {
                        Some(req.tags)
                    } else {
                        None
                    },
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(cert.into()))
    }

    async fn delete_ca_cert(
        &self,
        request: Request<DeleteCaCertRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let deleted = self
            .store
            .delete_ca_cert(&req.id)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(DeleteResponse {
            success: deleted,
            message: if deleted {
                "CA certificate deleted".to_string()
            } else {
                "CA certificate not found".to_string()
            },
        }))
    }
}

// ============================================================================
// 6. DatabaseService Implementation
// ============================================================================

#[tonic::async_trait]
impl DatabaseService for GrpcSolidService {
    async fn list_databases(
        &self,
        _request: Request<ListDatabasesRequest>,
    ) -> Result<Response<ListDatabasesResponse>, Status> {
        let dbs = self.store.list_databases().await.map_err(Status::from)?;
        Ok(Response::new(ListDatabasesResponse {
            databases: dbs.into_iter().map(Into::into).collect(),
        }))
    }

    async fn create_database(
        &self,
        request: Request<CreateDatabaseRequest>,
    ) -> Result<Response<DatabaseMetaMsg>, Status> {
        let req = request.into_inner();
        let meta = self
            .store
            .create_database(CreateDatabaseInput {
                name: req.name,
                owner: if req.owner.is_empty() {
                    None
                } else {
                    Some(req.owner)
                },
                tags: req.tags,
            })
            .await
            .map_err(Status::from)?;

        Ok(Response::new(meta.into()))
    }

    async fn update_database_meta(
        &self,
        request: Request<UpdateDatabaseMetaRequest>,
    ) -> Result<Response<DatabaseMetaMsg>, Status> {
        let req = request.into_inner();
        let meta = self
            .store
            .update_database_meta(
                &req.name,
                UpdateDatabaseMetaInput {
                    owner: req.owner,
                    tags: Some(req.tags),
                },
            )
            .await
            .map_err(Status::from)?;

        Ok(Response::new(meta.into()))
    }

    async fn execute_query(
        &self,
        request: Request<ExecuteQueryRequest>,
    ) -> Result<Response<ExecuteQueryResponse>, Status> {
        let req = request.into_inner();
        let vars = if req.vars_json.is_empty() {
            None
        } else {
            Some(req.vars_json.as_str())
        };
        let result = self
            .store
            .execute_query(&req.query, vars)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ExecuteQueryResponse {
            result_json: serde_json::to_string(&result).unwrap_or_else(|_| "null".into()),
        }))
    }
}
