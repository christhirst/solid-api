use solid_api::grpc::GrpcSolidService;
use solid_api::grpc::pb::connection_service_server::ConnectionService;
use solid_api::grpc::pb::dashboard_service_server::DashboardService;
use solid_api::grpc::pb::database_service_server::DatabaseService;
use solid_api::grpc::pb::registry_service_server::RegistryService;
use solid_api::grpc::pb::user_provisioning_service_server::UserProvisioningService;
use solid_api::grpc::pb::workflow_service_server::WorkflowService;
use solid_api::grpc::pb::*;
use solid_api::store::Store;
use tonic::Request;

#[tokio::test]
async fn test_grpc_user_group_and_db_user_service() {
    let store = Store::new_in_memory().await.expect("in-memory store");
    let grpc = GrpcSolidService::new(store);

    // 0. Init schema via gRPC
    let init_res = grpc
        .init_schema(Request::new(Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert!(init_res.success);

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

    // 2. Get user & Effective Access
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

    // 4. Create Group & Assign Member
    let grp_req = Request::new(CreateGroupRequest {
        name: "cloud.platform".into(),
        owner: "grpc_user_01".into(),
        description: "Cloud Platform Engineering".into(),
    });
    let group = grpc.create_group(grp_req).await.unwrap().into_inner();
    assert_eq!(group.name, "cloud.platform");
    assert_eq!(group.parent, "cloud");

    let access = grpc
        .get_effective_access(Request::new(GetUserRequest {
            sub_or_username: "frank".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(access.platform_role, "creator");
    assert!(access.capabilities.unwrap().can_manage_workflows);

    // 5. Delete User
    let del_req = Request::new(DeleteUserRequest {
        sub_or_username: "frank".into(),
    });
    let del_res = grpc.delete_user(del_req).await.unwrap().into_inner();
    assert!(del_res.success);
}

#[tokio::test]
async fn test_grpc_dashboards_workflows_connections_registry_and_database() {
    let store = Store::new_in_memory().await.expect("in-memory store");
    let grpc = GrpcSolidService::new(store);

    // Provision owner user first
    grpc.provision_user(Request::new(ProvisionUserRequest {
        sub: "usr_owner".into(),
        username: "sara".into(),
        name: "Sara".into(),
        email: "sara@example.com".into(),
        role: "admin".into(),
        namespace: "sara-space".into(),
    }))
    .await
    .unwrap();

    // ------------------------------------------------------------------------
    // 1. DashboardService
    // ------------------------------------------------------------------------
    let dash = grpc
        .create_dashboard(Request::new(CreateDashboardRequest {
            id: "dash_ops".into(),
            name: "Operations Overview".into(),
            description: "Main SRE dashboard".into(),
            owner: "usr_owner".into(),
            slug: "ops-overview".into(),
            is_public: true,
            tags: vec!["sre".into(), "prod".into()],
            buttons_json:
                r#"[{"id":"btn1","label":"Check Health","type":"stat","x":0,"y":0,"w":4,"h":2}]"#
                    .into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(dash.id, "dashboard:dash_ops");
    assert_eq!(dash.namespace, "sara-space");
    assert!(dash.is_public);
    assert!(dash.buttons_json.contains("Check Health"));

    // SaveUserNamespace cascades to owned dashboards
    let ns_res = grpc
        .save_user_namespace(Request::new(SaveUserNamespaceRequest {
            sub_or_username: "sara".into(),
            namespace: "sara-new-ns".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ns_res.namespace, "sara-new-ns");

    let fetched_dash = grpc
        .get_dashboard(Request::new(GetDashboardRequest {
            id: "dash_ops".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(fetched_dash.namespace, "sara-new-ns");

    // ------------------------------------------------------------------------
    // 2. WorkflowService
    // ------------------------------------------------------------------------
    let wf = grpc
        .create_workflow(Request::new(CreateWorkflowRequest {
            id: "wf_deploy".into(),
            name: "Deploy Pipeline".into(),
            description: "Automated deployment workflow".into(),
            owner: "usr_owner".into(),
            schedule: "0 */6 * * *".into(),
            server_address: "localhost:50051".into(),
            use_tls: false,
            direction: "lr".into(),
            visibility: "group".into(),
            tags: vec!["sre".into()],
            steps_json: r#"[{"id":"s1","name":"Run Check","stepType":"grpc"}]"#.into(),
            graph_json: r#"{"nodes":[{"id":"s1"}],"edges":[]}"#.into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(wf.id, "workflow:wf_deploy");
    assert_eq!(wf.direction, "lr");

    // Clone Workflow
    let cloned_wf = grpc
        .clone_workflow(Request::new(CloneWorkflowRequest {
            source_id: "wf_deploy".into(),
            new_name: "Deploy Pipeline Staging".into(),
            owner: "usr_owner".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(cloned_wf.name, "Deploy Pipeline Staging");
    assert_ne!(cloned_wf.id, wf.id);

    // ------------------------------------------------------------------------
    // 3. ConnectionService
    // ------------------------------------------------------------------------
    let conn = grpc
        .create_connection(Request::new(CreateConnectionRequest {
            id: "conn_grpc_prod".into(),
            name: "Prod gRPC Endpoint".into(),
            conn_type: "grpc".into(),
            description: "Production microservice".into(),
            owner: "usr_owner".into(),
            visibility: "group".into(),
            tags: vec!["prod".into()],
            config_json: r#"{"serverAddress":"api.prod.internal:50051","useTls":true}"#.into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(conn.id, "connection:conn_grpc_prod");
    assert_eq!(conn.conn_type, "grpc");
    assert!(conn.config_json.contains("api.prod.internal:50051"));

    let grpc_conns = grpc
        .list_connections(Request::new(ListConnectionsRequest {
            conn_type: "grpc".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(grpc_conns.connections.len(), 1);

    // ------------------------------------------------------------------------
    // 4. RegistryService (Proto Files & Trusted CA Certificates)
    // ------------------------------------------------------------------------
    let proto = grpc
        .upload_proto(Request::new(UploadProtoRequest {
            id: "proto_orders".into(),
            name: "orders.proto".into(),
            content: "syntax = \"proto3\"; package orders.v1;".into(),
            owner: "usr_owner".into(),
            visibility: "public".into(),
            tags: vec!["orders".into()],
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(proto.id, "proto_file:proto_orders");
    assert_eq!(proto.name, "orders.proto");

    let cert = grpc
        .upload_ca_cert(Request::new(UploadCaCertRequest {
            id: "ca_internal".into(),
            name: "Internal Root CA".into(),
            content: "-----BEGIN CERTIFICATE-----\nMIIB...\n-----END CERTIFICATE-----".into(),
            owner: "usr_owner".into(),
            visibility: "group".into(),
            tags: vec!["security".into()],
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(cert.id, "ca_cert:ca_internal");

    let certs_list = grpc
        .list_ca_certs(Request::new(ListCaCertsRequest {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(certs_list.certs.len(), 1);

    // ------------------------------------------------------------------------
    // 5. DatabaseService
    // ------------------------------------------------------------------------
    let db_meta = grpc
        .create_database(Request::new(CreateDatabaseRequest {
            name: "analytics_db".into(),
            owner: "usr_owner".into(),
            tags: vec!["analytics".into()],
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(db_meta.name, "analytics_db");

    let query_res = grpc
        .execute_query(Request::new(ExecuteQueryRequest {
            query: "SELECT name, isPublic FROM dashboard WHERE slug = $slug;".into(),
            vars_json: r#"{"slug":"ops-overview"}"#.into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(query_res.result_json.contains("Operations Overview"));
}
