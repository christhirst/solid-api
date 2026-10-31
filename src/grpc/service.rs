use tonic::{Request, Response, Status};

use crate::grpc::pb::user_provisioning_service_server::UserProvisioningService;
use crate::grpc::pb::*;
use crate::models::{
    AssignGroupMemberRequest as ModelAssignGroupMemberRequest,
    CreateGroupRequest as ModelCreateGroupRequest, GroupMember, GroupNamespace, GroupRight,
    PlatformRole, ProvisionDbUserRequest as ModelProvisionDbUserRequest,
    ProvisionUserRequest as ModelProvisionUserRequest, UserProfile,
};
use crate::store::Store;

#[derive(Debug, Clone)]
pub struct GrpcUserProvisioningService {
    store: Store,
}

impl GrpcUserProvisioningService {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

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

#[tonic::async_trait]
impl UserProvisioningService for GrpcUserProvisioningService {
    async fn provision_user(
        &self,
        request: Request<ProvisionUserRequest>,
    ) -> Result<Response<UserProfileMsg>, Status> {
        let req = request.into_inner();
        let role = if !req.role.is_empty() {
            PlatformRole::from_str_loose(&req.role)
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

    async fn list_groups(
        &self,
        _request: Request<ListGroupsRequest>,
    ) -> Result<Response<ListGroupsResponse>, Status> {
        let groups = self.store.list_groups().await.map_err(Status::from)?;
        Ok(Response::new(ListGroupsResponse {
            groups: groups.into_iter().map(Into::into).collect(),
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
}
