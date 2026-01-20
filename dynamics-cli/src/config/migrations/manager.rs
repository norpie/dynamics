//! Migration manager for running up/down migrations

use anyhow::{Context, Result};
use log::{debug, info, warn};
use sqlx::SqlitePool;

use super::{
    AppliedMigration, Direction, Migration, calculate_checksum, get_applied_migrations,
    get_current_version, get_pending_migrations, init_migration_table, load_migrations,
    validate_migrations,
};

/// Migration manager handles running migrations up and down
pub struct MigrationManager<'a> {
    pool: &'a SqlitePool,
}

impl<'a> MigrationManager<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the migration system
    pub async fn init(&self) -> Result<()> {
        debug!("Initializing migration system");
        init_migration_table(self.pool).await?;
        Ok(())
    }

    /// Run all pending migrations
    pub async fn migrate_up(&self) -> Result<()> {
        self.init().await?;
        validate_migrations(self.pool).await?;

        let pending = get_pending_migrations(self.pool).await?;
        if pending.is_empty() {
            info!("No pending migrations");
            return Ok(());
        }

        info!("Running {} pending migrations", pending.len());
        for migration in pending {
            self.apply_migration(&migration, Direction::Up).await?;
        }

        info!("All migrations completed successfully");
        Ok(())
    }

    /// Rollback to a specific version (or all the way down if None)
    pub async fn migrate_down(&self, target_version: Option<i64>) -> Result<()> {
        self.init().await?;
        validate_migrations(self.pool).await?;

        let applied = get_applied_migrations(self.pool).await?;
        let available = load_migrations()?;

        let target = target_version.unwrap_or(0);
        let current = get_current_version(self.pool).await?.unwrap_or(0);

        if target >= current {
            info!("Already at or below target version {}", target);
            return Ok(());
        }

        // Get migrations to rollback (in reverse order)
        let mut to_rollback = Vec::new();
        for applied_migration in applied.into_iter().rev() {
            if applied_migration.version > target {
                if let Some(migration) = available.get(&applied_migration.version) {
                    to_rollback.push(migration.clone());
                } else {
                    anyhow::bail!(
                        "Cannot rollback migration {} - migration file not found",
                        applied_migration.version
                    );
                }
            }
        }

        if to_rollback.is_empty() {
            info!("No migrations to rollback");
            return Ok(());
        }

        info!(
            "Rolling back {} migrations to version {}",
            to_rollback.len(),
            target
        );
        for migration in to_rollback {
            self.apply_migration(&migration, Direction::Down).await?;
        }

        info!("Rollback completed successfully");
        Ok(())
    }

    /// Apply a single migration in the specified direction
    async fn apply_migration(&self, migration: &Migration, direction: Direction) -> Result<()> {
        let sql = match direction {
            Direction::Up => &migration.up_sql,
            Direction::Down => &migration.down_sql,
        };

        if sql.trim().is_empty() {
            warn!(
                "Migration {} has empty {} SQL, skipping",
                migration.version,
                match direction {
                    Direction::Up => "up",
                    Direction::Down => "down",
                }
            );
            return Ok(());
        }

        info!(
            "{} migration {} '{}'",
            match direction {
                Direction::Up => "Applying",
                Direction::Down => "Rolling back",
            },
            migration.version,
            migration.name
        );

        debug!("Executing SQL:\n{}", sql);

        // Start transaction
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start migration transaction")?;

        // Execute the migration SQL as a single statement
        // SQLite can handle multiple statements separated by semicolons
        if !sql.trim().is_empty() {
            sqlx::query(sql).execute(&mut *tx).await.with_context(|| {
                format!(
                    "Failed to execute migration {} {} SQL",
                    migration.version,
                    match direction {
                        Direction::Up => "up",
                        Direction::Down => "down",
                    }
                )
            })?;
        }

        // Update migration tracking
        match direction {
            Direction::Up => {
                let checksum = calculate_checksum(&migration.up_sql);
                sqlx::query(
                    "INSERT INTO schema_migrations (version, name, checksum) VALUES (?, ?, ?)",
                )
                .bind(migration.version)
                .bind(&migration.name)
                .bind(&checksum)
                .execute(&mut *tx)
                .await
                .context("Failed to record migration")?;
            }
            Direction::Down => {
                sqlx::query("DELETE FROM schema_migrations WHERE version = ?")
                    .bind(migration.version)
                    .execute(&mut *tx)
                    .await
                    .context("Failed to remove migration record")?;
            }
        }

        // Commit transaction
        tx.commit()
            .await
            .context("Failed to commit migration transaction")?;

        info!(
            "Migration {} {} completed",
            migration.version,
            match direction {
                Direction::Up => "applied",
                Direction::Down => "rolled back",
            }
        );

        Ok(())
    }

    /// Get migration status
    pub async fn status(&self) -> Result<MigrationStatus> {
        self.init().await?;

        let available = load_migrations()?;
        let applied = get_applied_migrations(self.pool).await?;
        let pending = get_pending_migrations(self.pool).await?;
        let current_version = get_current_version(self.pool).await?;

        Ok(MigrationStatus {
            current_version,
            total_available: available.len(),
            applied_count: applied.len(),
            pending_count: pending.len(),
            applied_migrations: applied,
            pending_migrations: pending,
        })
    }

    /// Validate that all applied migrations are consistent
    pub async fn validate(&self) -> Result<()> {
        self.init().await?;
        validate_migrations(self.pool).await?;
        info!("All applied migrations are valid");
        Ok(())
    }
}

/// Migration status information
#[derive(Debug)]
pub struct MigrationStatus {
    pub current_version: Option<i64>,
    pub total_available: usize,
    pub applied_count: usize,
    pub pending_count: usize,
    pub applied_migrations: Vec<AppliedMigration>,
    pub pending_migrations: Vec<Migration>,
}

impl MigrationStatus {
    pub fn is_up_to_date(&self) -> bool {
        self.pending_count == 0
    }

    pub fn print_status(&self) {
        println!("Migration Status:");
        println!("  Current version: {:?}", self.current_version);
        println!("  Applied migrations: {}", self.applied_count);
        println!("  Pending migrations: {}", self.pending_count);
        println!("  Total available: {}", self.total_available);
        println!("  Up to date: {}", self.is_up_to_date());

        if !self.applied_migrations.is_empty() {
            println!("\nApplied migrations:");
            for migration in &self.applied_migrations {
                println!(
                    "  ✓ {} {} ({})",
                    migration.version,
                    migration.name,
                    migration.applied_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }

        if !self.pending_migrations.is_empty() {
            println!("\nPending migrations:");
            for migration in &self.pending_migrations {
                println!("  ○ {} {}", migration.version, migration.name);
            }
        }
    }
}
