use std::path::Path;

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub surrealdb: SurrealDbConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SurrealDbConfig {
    pub endpoint: String,
    pub namespace: String,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                listen_addr: "0.0.0.0:50051".to_string(),
            },
            surrealdb: SurrealDbConfig {
                endpoint: "wss://app.ux-ti.com/rpc".to_string(),
                namespace: "solidflow".to_string(),
                database: "main".to_string(),
                username: Some("solid".to_string()),
                password: Some("sol1d".to_string()),
            },
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from_dir(Path::new("config"))
    }

    pub fn load_from_dir(config_dir: &Path) -> Result<Self, ConfigError> {
        let default_file = config_dir.join("default");
        let local_file = config_dir.join("local");

        let mut builder = Config::builder()
            .set_default("server.listen_addr", "0.0.0.0:50051")?
            .set_default("surrealdb.endpoint", "wss://app.ux-ti.com/rpc")?
            .set_default("surrealdb.namespace", "solidflow")?
            .set_default("surrealdb.database", "main")?
            .set_default("surrealdb.username", "solid")?
            .set_default("surrealdb.password", "sol1d")?
            .add_source(File::from(default_file).required(false))
            .add_source(File::from(local_file).required(false));

        if let Ok(custom_path) = std::env::var("SOLID_API_CONFIG_PATH") {
            builder = builder.add_source(File::with_name(&custom_path).required(true));
        }

        // Read direct SURREALDB_* environment variables if present
        if let Ok(url) = std::env::var("SURREALDB_URL") {
            builder = builder.set_override("surrealdb.endpoint", url)?;
        }
        if let Ok(ns) = std::env::var("SURREALDB_NS") {
            builder = builder.set_override("surrealdb.namespace", ns)?;
        }
        if let Ok(db) = std::env::var("SURREALDB_DB") {
            builder = builder.set_override("surrealdb.database", db)?;
        }
        if let Ok(user) = std::env::var("SURREALDB_USER") {
            builder = builder.set_override("surrealdb.username", user)?;
        }
        if let Ok(pass) = std::env::var("SURREALDB_PASS") {
            builder = builder.set_override("surrealdb.password", pass)?;
        }

        // Server port override
        if let Ok(grpc_port) = std::env::var("GRPC_PORT") {
            builder = builder.set_override("server.listen_addr", format!("0.0.0.0:{grpc_port}"))?;
        }

        // Allow SOLID_API__* env overrides (e.g. SOLID_API_SERVER__LISTEN_ADDR)
        builder = builder.add_source(
            Environment::with_prefix("SOLID_API")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        );

        builder.build()?.try_deserialize()
    }
}
