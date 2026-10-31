use clap::Parser;
use solid_api::api::build_app;
use solid_api::cli::{
    Cli, Commands, DbUserCommands, GroupCommands, SchemaCommands, ServeArgs, UserCommands,
};
use solid_api::config::AppConfig;
use solid_api::error::AppError;
use solid_api::grpc::GrpcUserProvisioningService;
use solid_api::grpc::pb::user_provisioning_service_server::UserProvisioningServiceServer;
use solid_api::models::{
    AssignGroupMemberRequest, CreateGroupRequest, GroupRight, PlatformRole, ProvisionDbUserRequest,
    ProvisionUserRequest,
};
use solid_api::schema::ensure_schema;
use solid_api::store::Store;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
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

    match cli.command {
        Commands::Serve(serve_args) => {
            run_server(store, config, serve_args).await?;
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
    }

    Ok(())
}

async fn run_server(
    store: Store,
    config: AppConfig,
    args: ServeArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let http_addr_str = args.http_addr.unwrap_or(config.server.http_addr);
    let grpc_addr_str = args.grpc_addr.unwrap_or(config.server.grpc_addr);

    let http_addr: SocketAddr = http_addr_str.parse()?;
    let grpc_addr: SocketAddr = grpc_addr_str.parse()?;

    tracing::info!("Starting solid-api REST server on http://{http_addr}");
    tracing::info!("Starting solid-api gRPC server on {grpc_addr}");

    let app = build_app(store.clone());
    let listener = tokio::net::TcpListener::bind(http_addr).await?;

    let http_server = async move {
        axum::serve(listener, app).await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    let grpc_service = GrpcUserProvisioningService::new(store);
    let grpc_server = async move {
        tonic::transport::Server::builder()
            .add_service(UserProvisioningServiceServer::new(grpc_service))
            .serve(grpc_addr)
            .await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    tokio::select! {
        res = http_server => {
            if let Err(e) = res {
                tracing::error!("HTTP server error: {e}");
            }
        }
        res = grpc_server => {
            if let Err(e) = res {
                tracing::error!("gRPC server error: {e}");
            }
        }
    }

    Ok(())
}
