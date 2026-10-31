use solid_api::grpc::GrpcUserProvisioningService;
use solid_api::grpc::pb::user_provisioning_service_server::UserProvisioningService;
use solid_api::grpc::pb::*;
use solid_api::store::Store;
use tonic::Request;

#[tokio::test]
async fn test_grpc_service_provisioning() {
    let store = Store::new_in_memory().await.expect("in-memory store");
    let grpc = GrpcUserProvisioningService::new(store);

    // 1. Provision user via gRPC
    let prov_req = Request::new(ProvisionUserRequest {
        sub: "grpc_user_01".into(),
        username: "frank".into(),
        name: "Frank".into(),
        email: "frank@example.com".into(),
        role: "user".into(),
        namespace: "frank-ns".into(),
    });
    let user_res = grpc.provision_user(prov_req).await.unwrap().into_inner();
    assert_eq!(user_res.username, "frank");
    assert_eq!(user_res.idp_role, "user");

    // 2. Get user
    let get_req = Request::new(GetUserRequest {
        sub_or_username: "frank".into(),
    });
    let fetched = grpc.get_user(get_req).await.unwrap().into_inner();
    assert_eq!(fetched.sub, "grpc_user_01");

    // 3. Assign Platform Role
    let role_req = Request::new(AssignPlatformRoleRequest {
        sub_or_username: "frank".into(),
        role: "creator".into(),
        as_override: false,
    });
    let updated = grpc
        .assign_platform_role(role_req)
        .await
        .unwrap()
        .into_inner();
    assert_eq!(updated.idp_role, "creator");

    // 4. Create Group
    let grp_req = Request::new(CreateGroupRequest {
        name: "cloud".into(),
        owner: "grpc_user_01".into(),
        description: "Cloud Engineering".into(),
    });
    let group = grpc.create_group(grp_req).await.unwrap().into_inner();
    assert_eq!(group.name, "cloud");

    // 5. Delete User
    let del_req = Request::new(DeleteUserRequest {
        sub_or_username: "frank".into(),
    });
    let del_res = grpc.delete_user(del_req).await.unwrap().into_inner();
    assert!(del_res.success);
}
