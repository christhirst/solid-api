use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "solid-api",
    about = "SolidFlow gRPC Management Service & Model Context Protocol (MCP) Server",
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
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the Tonic gRPC API server (default if no subcommand is given)
    Serve(ServeArgs),

    /// Start the Model Context Protocol (MCP) JSON-RPC 2.0 server over stdio
    Mcp,

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

    /// Dashboard management
    Dashboard {
        #[command(subcommand)]
        cmd: DashboardCommands,
    },

    /// Workflow management
    Workflow {
        #[command(subcommand)]
        cmd: WorkflowCommands,
    },

    /// Connection profile management
    Connection {
        #[command(subcommand)]
        cmd: ConnectionCommands,
    },

    /// Proto registry management
    Proto {
        #[command(subcommand)]
        cmd: ProtoCommands,
    },

    /// Trusted CA certificate management
    Ca {
        #[command(subcommand)]
        cmd: CaCommands,
    },
}

#[derive(Debug, Args, Default, Clone)]
pub struct ServeArgs {
    #[arg(
        long,
        alias = "grpc-addr",
        help = "gRPC server bind address (e.g. 0.0.0.0:50051)"
    )]
    pub listen_addr: Option<String>,
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

#[derive(Debug, Subcommand)]
pub enum DashboardCommands {
    /// List dashboards
    List,
    /// Get dashboard by ID
    Get { id: String },
    /// Delete dashboard by ID
    Delete { id: String },
}

#[derive(Debug, Subcommand)]
pub enum WorkflowCommands {
    /// List workflows
    List,
    /// Get workflow by ID
    Get { id: String },
    /// Delete workflow by ID
    Delete { id: String },
}

#[derive(Debug, Subcommand)]
pub enum ConnectionCommands {
    /// List connections
    List,
    /// Get connection by ID
    Get { id: String },
    /// Delete connection by ID
    Delete { id: String },
}

#[derive(Debug, Subcommand)]
pub enum ProtoCommands {
    /// List proto files
    List,
    /// Get proto file by ID or name
    Get { id: String },
    /// Upload a proto file from disk
    Upload {
        #[arg(long)]
        name: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        owner: Option<String>,
    },
    /// Delete proto file by ID
    Delete { id: String },
}

#[derive(Debug, Subcommand)]
pub enum CaCommands {
    /// List trusted CA certificates
    List,
    /// Get trusted CA certificate by ID or name
    Get { id: String },
    /// Upload a PEM CA certificate from disk
    Upload {
        #[arg(long)]
        name: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        owner: Option<String>,
    },
    /// Delete trusted CA certificate by ID
    Delete { id: String },
}
