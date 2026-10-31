use clap::Parser;
use solid_api::cli::{
    CaCommands, Cli, Commands, ConnectionCommands, DashboardCommands, DbUserCommands,
    GroupCommands, ProtoCommands, SchemaCommands, ServeArgs, UserCommands, WorkflowCommands,
};
use solid_api::config::AppConfig;
use solid_api::error::AppError;
use solid_api::grpc::GrpcSolidService;
use solid_api::grpc::pb::connection_service_server::ConnectionServiceServer;
use solid_api::grpc::pb::dashboard_service_server::DashboardServiceServer;
use solid_api::grpc::pb::database_service_server::DatabaseServiceServer;
use solid_api::grpc::pb::registry_service_server::RegistryServiceServer;
use solid_api::grpc::pb::user_provisioning_service_server::UserProvisioningServiceServer;
use solid_api::grpc::pb::workflow_service_server::WorkflowServiceServer;
use solid_api::mcp::run_mcp_stdio;
use solid_api::models::{
    AssignGroupMemberRequest, CreateGroupRequest, GroupRight, PlatformRole, ProvisionDbUserRequest,
    ProvisionUserRequest, UploadCaCertInput, UploadProtoInput,
};
use solid_api::schema::ensure_schema;
use solid_api::store::Store;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Log to stderr so stdout remains clean for `solid-api mcp` JSON-RPC 2.0 communication
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,solid_api=debug")),
        )
        .init();

    let cli = Cli::parse();
    let mut config = AppConfig::load().unwrap_or_default();

    // CLI overrides for SurrealDB connection
    if let Some(endpoint) = cli.endpoint {
        config.surrealdb.endpoint = endpoint;
    }
    if let Some(namespace) = cli.namespace {
        config.surrealdb.namespace = namespace;
    }
    if let Some(database) = cli.database {
        config.surrealdb.database = database;
    }
    if let Some(username) = cli.username {
        config.surrealdb.username = Some(username);
    }
    if let Some(password) = cli.password {
        config.surrealdb.password = Some(password);
    }

    let store = Store::connect(&config.surrealdb).await?;
    let command = cli
        .command
        .unwrap_or_else(|| Commands::Serve(ServeArgs::default()));

    match command {
        Commands::Serve(serve_args) => {
            run_server(store, config, serve_args).await?;
        }
        Commands::Mcp => {
            tracing::info!("Starting solid-api MCP server over stdio");
            run_mcp_stdio(store).await?;
        }
        Commands::Schema { cmd } => match cmd {
            SchemaCommands::Init => {
                ensure_schema(&store.db).await?;
                println!("SurrealDB schema initialized successfully.");
            }
        },
        Commands::User { cmd } => match cmd {
            UserCommands::Provision {
                sub,
                username,
                name,
                email,
                role,
                namespace,
            } => {
                let role = role.as_deref().and_then(PlatformRole::from_str_loose);
                let user = store
                    .provision_user(ProvisionUserRequest {
                        sub,
                        username,
                        name,
                        email,
                        role,
                        namespace,
                    })
                    .await?;
                println!("{}", serde_json::to_string_pretty(&user)?);
            }
            UserCommands::Get { sub_or_username } => {
                let user = store.get_user(&sub_or_username).await?;
                println!("{}", serde_json::to_string_pretty(&user)?);
            }
            UserCommands::List { limit } => {
                let users = store.list_users(limit).await?;
                println!("{}", serde_json::to_string_pretty(&users)?);
            }
            UserCommands::AssignRole {
                sub_or_username,
                role,
                as_override,
            } => {
                let role_enum = PlatformRole::from_str_loose(&role).ok_or_else(|| {
                    AppError::InvalidArgument(format!("Invalid platform role '{role}'"))
                })?;
                let user = store
                    .update_user_role(&sub_or_username, role_enum, as_override)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&user)?);
            }
            UserCommands::Delete { sub_or_username } => {
                let deleted = store.delete_user(&sub_or_username).await?;
                if deleted {
                    println!("User '{sub_or_username}' deleted successfully.");
                } else {
                    println!("User '{sub_or_username}' not found.");
                }
            }
            UserCommands::Access { sub_or_username } => {
                let ctx = store.get_effective_access(&sub_or_username).await?;
                println!("{}", serde_json::to_string_pretty(&ctx)?);
            }
        },
        Commands::Group { cmd } => match cmd {
            GroupCommands::Create {
                name,
                owner,
                description,
            } => {
                let group = store
                    .create_group(CreateGroupRequest {
                        name,
                        owner,
                        description,
                    })
                    .await?;
                println!("{}", serde_json::to_string_pretty(&group)?);
            }
            GroupCommands::Get { name } => {
                let group = store.get_group(&name).await?;
                println!("{}", serde_json::to_string_pretty(&group)?);
            }
            GroupCommands::List => {
                let groups = store.list_groups().await?;
                println!("{}", serde_json::to_string_pretty(&groups)?);
            }
            GroupCommands::Delete { name } => {
                let deleted = store.delete_group(&name).await?;
                if deleted {
                    println!("Group '{name}' deleted successfully.");
                } else {
                    println!("Group '{name}' not found.");
                }
            }
            GroupCommands::Assign {
                group,
                username,
                rights,
                assigned_by,
            } => {
                let rights_vec: Vec<GroupRight> = rights
                    .split(',')
                    .filter_map(GroupRight::from_str_loose)
                    .collect();
                let member = store
                    .assign_group_role(
                        &group,
                        AssignGroupMemberRequest {
                            username,
                            rights: rights_vec,
                            assigned_by,
                        },
                    )
                    .await?;
                println!("{}", serde_json::to_string_pretty(&member)?);
            }
            GroupCommands::RemoveMember { group, username } => {
                let removed = store.remove_group_member(&group, &username).await?;
                if removed {
                    println!("Member '{username}' removed from group '{group}'.");
                } else {
                    println!("Member '{username}' not found in group '{group}'.");
                }
            }
            GroupCommands::Members { name } => {
                let members = store.list_group_members(&name).await?;
                println!("{}", serde_json::to_string_pretty(&members)?);
            }
        },
        Commands::DbUser { cmd } => match cmd {
            DbUserCommands::Provision {
                username,
                password,
                roles,
            } => {
                let roles_vec: Vec<String> =
                    roles.split(',').map(|s| s.trim().to_uppercase()).collect();
                let user = store
                    .provision_db_user(ProvisionDbUserRequest {
                        username,
                        password,
                        roles: Some(roles_vec),
                    })
                    .await?;
                println!("{}", serde_json::to_string_pretty(&user)?);
            }
            DbUserCommands::List => {
                let users = store.list_db_users().await?;
                println!("{}", serde_json::to_string_pretty(&users)?);
            }
            DbUserCommands::Remove { username } => {
                store.remove_db_user(&username).await?;
                println!("Database user '{username}' removed.");
            }
        },
        Commands::Dashboard { cmd } => match cmd {
            DashboardCommands::List => {
                let list = store.list_dashboards(None, false).await?;
                println!("{}", serde_json::to_string_pretty(&list)?);
            }
            DashboardCommands::Get { id } => {
                let item = store.get_dashboard(&id).await?;
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
            DashboardCommands::Delete { id } => {
                let deleted = store.delete_dashboard(&id).await?;
                println!("{}", serde_json::json!({ "deleted": deleted }));
            }
        },
        Commands::Workflow { cmd } => match cmd {
            WorkflowCommands::List => {
                let list = store.list_workflows(None).await?;
                println!("{}", serde_json::to_string_pretty(&list)?);
            }
            WorkflowCommands::Get { id } => {
                let item = store.get_workflow(&id).await?;
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
            WorkflowCommands::Delete { id } => {
                let deleted = store.delete_workflow(&id).await?;
                println!("{}", serde_json::json!({ "deleted": deleted }));
            }
        },
        Commands::Connection { cmd } => match cmd {
            ConnectionCommands::List => {
                let list = store.list_connections(None).await?;
                println!("{}", serde_json::to_string_pretty(&list)?);
            }
            ConnectionCommands::Get { id } => {
                let item = store.get_connection(&id).await?;
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
            ConnectionCommands::Delete { id } => {
                let deleted = store.delete_connection(&id).await?;
                println!("{}", serde_json::json!({ "deleted": deleted }));
            }
        },
        Commands::Proto { cmd } => match cmd {
            ProtoCommands::List => {
                let list = store.list_protos(None).await?;
                println!("{}", serde_json::to_string_pretty(&list)?);
            }
            ProtoCommands::Get { id } => {
                let item = store.get_proto(&id).await?;
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
            ProtoCommands::Upload { name, file, owner } => {
                let content = std::fs::read_to_string(&file)?;
                let item = store
                    .upload_proto(UploadProtoInput {
                        id: None,
                        name,
                        content,
                        owner,
                        visibility: Some("private".into()),
                        tags: Vec::new(),
                    })
                    .await?;
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
            ProtoCommands::Delete { id } => {
                let deleted = store.delete_proto(&id).await?;
                println!("{}", serde_json::json!({ "deleted": deleted }));
            }
        },
        Commands::Ca { cmd } => match cmd {
            CaCommands::List => {
                let list = store.list_ca_certs().await?;
                println!("{}", serde_json::to_string_pretty(&list)?);
            }
            CaCommands::Get { id } => {
                let item = store.get_ca_cert(&id).await?;
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
            CaCommands::Upload { name, file, owner } => {
                let content = std::fs::read_to_string(&file)?;
                let item = store
                    .upload_ca_cert(UploadCaCertInput {
                        id: None,
                        name,
                        content,
                        owner,
                        visibility: Some("private".into()),
                        tags: Vec::new(),
                    })
                    .await?;
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
            CaCommands::Delete { id } => {
                let deleted = store.delete_ca_cert(&id).await?;
                println!("{}", serde_json::json!({ "deleted": deleted }));
            }
        },
    }

    Ok(())
}

async fn run_server(
    store: Store,
    config: AppConfig,
    args: ServeArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr_str = args.listen_addr.unwrap_or(config.server.listen_addr);
    let addr: SocketAddr = addr_str.parse()?;

    tracing::info!("Starting solid-api Tonic gRPC server on {addr}");

    let svc = GrpcSolidService::new(store);

    tonic::transport::Server::builder()
        .add_service(UserProvisioningServiceServer::new(svc.clone()))
        .add_service(DashboardServiceServer::new(svc.clone()))
        .add_service(WorkflowServiceServer::new(svc.clone()))
        .add_service(ConnectionServiceServer::new(svc.clone()))
        .add_service(RegistryServiceServer::new(svc.clone()))
        .add_service(DatabaseServiceServer::new(svc))
        .serve(addr)
        .await?;

    Ok(())
}
