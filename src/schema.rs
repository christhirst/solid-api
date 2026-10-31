use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::error::AppError;

pub const INIT_SCHEMA_SQL: &str = r#"
DEFINE TABLE IF NOT EXISTS user_profile TYPE NORMAL SCHEMALESS PERMISSIONS NONE;
DEFINE TABLE IF NOT EXISTS group_namespace TYPE NORMAL SCHEMALESS PERMISSIONS NONE;
DEFINE TABLE IF NOT EXISTS group_member TYPE NORMAL SCHEMALESS PERMISSIONS NONE;
DEFINE INDEX IF NOT EXISTS user_namespace ON user_profile FIELDS namespace UNIQUE;
DEFINE INDEX IF NOT EXISTS group_member_idx ON group_member FIELDS group, username UNIQUE;
"#;

pub async fn ensure_schema(db: &Surreal<Any>) -> Result<(), AppError> {
    db.query(INIT_SCHEMA_SQL).await?.check()?;
    Ok(())
}
