//! Proper migration framework for configuration database

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub mod manager;

pub use manager::MigrationManager;

/// Represents a single migration with up and down SQL
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub name: String,
    pub up_sql: String,
    pub down_sql: String,
}

/// Migration status in the database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AppliedMigration {
    pub version: i64,
    pub name: String,
    pub applied_at: chrono::DateTime<chrono::Utc>,
    pub checksum: String,
}

/// Direction for migration operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// Load all available migrations from the embedded files
/// Migrations are auto-discovered from files/ directory using include_dir!
pub fn load_migrations() -> Result<BTreeMap<i64, Migration>> {
    use include_dir::{Dir, include_dir};

    // Embed the entire files directory at compile time
    static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/config/migrations/files");

    let mut migrations = BTreeMap::new();

    // Iterate through each migration directory (e.g., "001_initial", "002_indexes")
    for entry in MIGRATIONS_DIR.dirs() {
        let dir_name = entry
            .path()
            .to_str()
            .context("Invalid migration directory name")?;

        // Get just the directory name (last component)
        let name_only = entry
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid migration directory name")?;

        // Parse version and name from directory name (format: NNN_name)
        let parts: Vec<&str> = name_only.splitn(2, '_').collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "Invalid migration directory format: {}. Expected format: NNN_name",
                name_only
            );
        }

        let version: i64 = parts[0]
            .parse()
            .with_context(|| format!("Invalid migration version in directory: {}", name_only))?;
        let name = parts[1].to_string();

        // Read up.sql and down.sql from the directory
        // Files are stored with full paths like "001_initial/up.sql"
        let up_path = format!("{}/up.sql", name_only);
        let down_path = format!("{}/down.sql", name_only);

        let up_sql = MIGRATIONS_DIR
            .get_file(&up_path)
            .with_context(|| format!("Missing up.sql in migration {}", dir_name))?
            .contents_utf8()
            .with_context(|| format!("up.sql is not valid UTF-8 in migration {}", dir_name))?
            .to_string();

        let down_sql = MIGRATIONS_DIR
            .get_file(&down_path)
            .with_context(|| format!("Missing down.sql in migration {}", dir_name))?
            .contents_utf8()
            .with_context(|| format!("down.sql is not valid UTF-8 in migration {}", dir_name))?
            .to_string();

        migrations.insert(
            version,
            Migration {
                version,
                name,
                up_sql,
                down_sql,
            },
        );
    }

    if migrations.is_empty() {
        anyhow::bail!("No migrations found in files directory");
    }

    Ok(migrations)
}

/// Initialize the migration tracking table
pub async fn init_migration_table(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            checksum TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .context("Failed to create schema_migrations table")?;

    Ok(())
}

/// Get list of applied migrations
pub async fn get_applied_migrations(pool: &SqlitePool) -> Result<Vec<AppliedMigration>> {
    let migrations = sqlx::query_as::<_, AppliedMigration>(
        "SELECT version, name, applied_at, checksum FROM schema_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await
    .context("Failed to get applied migrations")?;

    Ok(migrations)
}

/// Calculate checksum for migration SQL
/// Normalizes line endings to LF before hashing to ensure cross-platform consistency
pub fn calculate_checksum(sql: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Normalize line endings by removing \r (Windows CRLF -> Unix LF)
    // This ensures the same checksum on Windows and Unix systems
    let normalized = sql.replace("\r\n", "\n").replace('\r', "\n");

    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Calculate checksum using the old method (without line ending normalization)
/// Used for backwards compatibility with databases that have old checksums
fn calculate_checksum_legacy(sql: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    sql.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Validate that applied migrations match available ones
pub async fn validate_migrations(pool: &SqlitePool) -> Result<()> {
    let available = load_migrations()?;
    let applied = get_applied_migrations(pool).await?;

    for applied_migration in applied {
        if let Some(available_migration) = available.get(&applied_migration.version) {
            let expected_checksum = calculate_checksum(&available_migration.up_sql);
            let legacy_checksum = calculate_checksum_legacy(&available_migration.up_sql);

            // Accept either the new (normalized) or legacy (raw) checksum for backwards compatibility
            let is_valid = applied_migration.checksum == expected_checksum
                || applied_migration.checksum == legacy_checksum;

            if !is_valid {
                anyhow::bail!(
                    "Migration {} checksum mismatch! Applied: {}, Expected: {} (or legacy: {}). \
                    This indicates the migration file has been modified after being applied.",
                    applied_migration.version,
                    applied_migration.checksum,
                    expected_checksum,
                    legacy_checksum
                );
            }
        } else {
            anyhow::bail!(
                "Applied migration {} '{}' not found in available migrations",
                applied_migration.version,
                applied_migration.name
            );
        }
    }

    Ok(())
}

/// Get pending migrations (available but not applied)
pub async fn get_pending_migrations(pool: &SqlitePool) -> Result<Vec<Migration>> {
    let available = load_migrations()?;
    let applied = get_applied_migrations(pool).await?;

    let applied_versions: std::collections::HashSet<i64> =
        applied.into_iter().map(|m| m.version).collect();

    let mut pending = Vec::new();
    for (version, migration) in available {
        if !applied_versions.contains(&version) {
            pending.push(migration);
        }
    }

    Ok(pending)
}

/// Get the current schema version (highest applied migration)
pub async fn get_current_version(pool: &SqlitePool) -> Result<Option<i64>> {
    let version: Option<(i64,)> = sqlx::query_as("SELECT MAX(version) FROM schema_migrations")
        .fetch_optional(pool)
        .await
        .context("Failed to get current schema version")?;

    Ok(version.and_then(|(v,)| if v == 0 { None } else { Some(v) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_migrations() {
        let migrations = load_migrations().unwrap();
        assert!(!migrations.is_empty(), "Should have at least one migration");

        // Should have migrations 1, 2, and 3 auto-discovered
        assert!(
            migrations.contains_key(&1),
            "Should have migration 001_initial"
        );
        assert!(
            migrations.contains_key(&2),
            "Should have migration 002_indexes"
        );
        assert!(
            migrations.contains_key(&3),
            "Should have migration 003_entity_cache"
        );

        // Verify each migration has up and down SQL
        for (version, migration) in &migrations {
            assert!(
                !migration.up_sql.is_empty(),
                "Migration {} should have up.sql",
                version
            );
            assert!(
                !migration.down_sql.is_empty(),
                "Migration {} should have down.sql",
                version
            );
            assert!(
                !migration.name.is_empty(),
                "Migration {} should have a name",
                version
            );
        }
    }

    #[test]
    fn test_calculate_checksum() {
        let sql = "CREATE TABLE test (id INTEGER);";
        let checksum1 = calculate_checksum(sql);
        let checksum2 = calculate_checksum(sql);
        assert_eq!(checksum1, checksum2);

        let different_sql = "CREATE TABLE test2 (id INTEGER);";
        let checksum3 = calculate_checksum(different_sql);
        assert_ne!(checksum1, checksum3);
    }
}
