use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "solid-api",
    about = "SolidFlow API & User Provisioning Service",
    version
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        help = "SurrealDB endpoint (e.g. wss://app.ux-ti.com/rpc, memory)"
    )]
    pub endpoint: Option<String>,

    #[arg(long, global = true, help = "SurrealDB namespace")]
    pub namespace: Option<String>,

    #[arg(long, global = true, help = "SurrealDB database")]
    pub database: Option<String>,

    #[arg(long, global = true, help = "SurrealDB username")]
    pub username: Option<String>,

    #[arg(long, global = true, help = "SurrealDB password")]
    pub password: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the HTTP REST and gRPC API servers
    Serve(ServeArgs),

    /// Schema management
    Schema {
        #[command(subcommand)]
        cmd: SchemaCommands,
    },

    /// User provisioning and management
    User {
        #[command(subcommand)]
        cmd: UserCommands,
    },

    /// Group / Space management
    Group {
        #[command(subcommand)]
        cmd: GroupCommands,
    },

    /// SurrealDB database-level users
    DbUser {
        #[command(subcommand)]
        cmd: DbUserCommands,
    },
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long, help = "HTTP server bind address")]
    pub http_addr: Option<String>,

    #[arg(long, help = "gRPC server bind address")]
    pub grpc_addr: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum SchemaCommands {
    /// Initialize SurrealDB tables and unique indexes
    Init,
}

#[derive(Debug, Subcommand)]
pub enum UserCommands {
    /// Provision a new user profile
    Provision {
        #[arg(long, help = "Subject ID (defaults to UUID if not specified)")]
        sub: Option<String>,

        #[arg(long, help = "Unique username")]
        username: String,

        #[arg(long, help = "Display name")]
        name: Option<String>,

        #[arg(long, help = "Email address")]
        email: Option<String>,

        #[arg(long, help = "Platform role (admin, creator, user, viewer)")]
        role: Option<String>,

        #[arg(long, help = "Personal publishing namespace")]
        namespace: Option<String>,
    },

    /// Get user details by sub or username
    Get {
        #[arg(help = "User sub or username")]
        sub_or_username: String,
    },

    /// List registered users
    List {
        #[arg(long, help = "Maximum users to return")]
        limit: Option<usize>,
    },

    /// Assign or change a user's platform role
    AssignRole {
        #[arg(help = "User sub or username")]
        sub_or_username: String,

        #[arg(long, help = "Role to assign (admin, creator, user, viewer)")]
        role: String,

        #[arg(long, help = "Set as roleOverride rather than idpRole")]
        as_override: bool,
    },

    /// Delete a user profile
    Delete {
        #[arg(help = "User sub or username")]
        sub_or_username: String,
    },

    /// Inspect effective access context and capabilities
    Access {
        #[arg(help = "User sub or username")]
        sub_or_username: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum GroupCommands {
    /// Create a new space / group namespace
    Create {
        #[arg(
            long,
            help = "Hierarchical group name (e.g. engineering, team.backend)"
        )]
        name: String,

        #[arg(long, help = "Owner sub or username")]
        owner: String,

        #[arg(long, help = "Optional description")]
        description: Option<String>,
    },

    /// Get group namespace details
    Get {
        #[arg(help = "Group name")]
        name: String,
    },

    /// List all groups
    List,

    /// Delete a group namespace and remove its members
    Delete {
        #[arg(help = "Group name")]
        name: String,
    },

    /// Assign a user to a group with specific rights
    Assign {
        #[arg(long, help = "Target group name")]
        group: String,

        #[arg(long, help = "Username or user ID to assign")]
        username: String,

        #[arg(
            long,
            default_value = "view,use",
            help = "Comma-separated rights (view,use,write,delete,manager)"
        )]
        rights: String,

        #[arg(long, help = "User performing the assignment")]
        assigned_by: Option<String>,
    },

    /// Remove a member from a group
    RemoveMember {
        #[arg(long, help = "Group name")]
        group: String,

        #[arg(long, help = "Username to remove")]
        username: String,
    },

    /// List members of a group
    Members {
        #[arg(help = "Group name")]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum DbUserCommands {
    /// Provision a SurrealDB database-level user
    Provision {
        #[arg(long, help = "DB Username")]
        username: String,

        #[arg(long, help = "DB Password")]
        password: String,

        #[arg(
            long,
            default_value = "OWNER",
            help = "Comma-separated roles (e.g. OWNER,EDITOR,VIEWER)"
        )]
        roles: String,
    },

    /// List SurrealDB database users
    List,

    /// Remove a SurrealDB database user
    Remove {
        #[arg(help = "DB Username")]
        username: String,
    },
}
