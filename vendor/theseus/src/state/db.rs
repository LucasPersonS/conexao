use crate::state::DirectoryInfo;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sha2::{Digest, Sha384};
use sqlx::migrate::Migrator;
use sqlx::{Pool, Sqlite};
use std::path::Path;
use std::time::Duration;

pub(crate) async fn connect(
    app_identifier: &str,
) -> crate::Result<Pool<Sqlite>> {
    let settings_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;

    crate::util::io::create_dir_all(&settings_dir).await?;

    let db_path = settings_dir.join("app.db");

    connect_app_db(&db_path).await
}

async fn connect_app_db(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    super::db_backup::maybe_backup_existing_app_db(db_path).await?;
    open_migrated_app_db(db_path).await
}

async fn open_migrated_app_db(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    let pool = open_app_db_pool(db_path).await?;

    if let Err(err) = stale_data_cleanup(&pool).await {
        tracing::warn!(
            "Failed to clean up stale data from state database before migrations: {err}"
        );
    }

    let migrator = sqlx::migrate!();
    repair_line_ending_checksums(&pool, &migrator).await?;
    migrator.run(&pool).await?;
    record_current_app_version(&pool).await?;

    if let Err(err) = stale_data_cleanup(&pool).await {
        tracing::warn!(
            "Failed to clean up stale data from state database: {err}"
        );
    }

    Ok(pool)
}

/// `sqlx::migrate!()` embeds each migration's SHA-384 at compile time. A build
/// made from a checkout with CRLF line endings embeds different checksums than
/// an LF build, and sqlx then refuses to start with "migration ... was
/// previously applied but has been modified". When the stored checksum matches
/// the same SQL with only the line endings swapped, update it to the current one.
async fn repair_line_ending_checksums(
    pool: &Pool<Sqlite>,
    migrator: &Migrator,
) -> crate::Result<()> {
    let has_table: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    if has_table == 0 {
        return Ok(());
    }

    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?;

    for (version, stored) in applied {
        let Some(migration) =
            migrator.iter().find(|m| m.version == version)
        else {
            continue;
        };
        if stored.as_slice() == &*migration.checksum {
            continue;
        }

        let lf = migration.sql.replace("\r\n", "\n");
        let crlf = lf.replace('\n', "\r\n");
        let only_line_endings_differ = [lf, crlf].iter().any(|sql| {
            Sha384::digest(sql.as_bytes())[..] == stored[..]
        });

        if only_line_endings_differ {
            tracing::warn!(
                "Repairing checksum of migration {version} (line endings changed)"
            );
            sqlx::query(
                "UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?",
            )
            .bind(&*migration.checksum)
            .bind(version)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

async fn open_app_db_pool(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    let conn_options = SqliteConnectOptions::new()
        .filename(db_path)
        .busy_timeout(Duration::from_secs(30))
        .journal_mode(SqliteJournalMode::Wal)
        .optimize_on_close(true, None)
        .create_if_missing(true);

    Ok(SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .idle_timeout(Duration::from_secs(120))
        .connect_with(conn_options)
        .await?)
}

async fn record_current_app_version(pool: &Pool<Sqlite>) -> crate::Result<()> {
    let previous_version = sqlx::query_scalar!(
        "SELECT value FROM app_metadata WHERE key = 'app_version'"
    )
    .fetch_optional(pool)
    .await?;
    let already_used_sync_update = previous_version
        .as_deref()
        .and_then(|version| {
            let mut parts = version.split('.');
            Some((
                parts.next()?.parse::<u64>().ok()?,
                parts.next()?.parse::<u64>().ok()?,
            ))
        })
        .is_some_and(|version| version >= (0, 20));

    if env!("CARGO_PKG_VERSION").starts_with("0.20.")
        && already_used_sync_update
    {
        let mut settings = super::Settings::get(pool).await?;
        if settings.pending_update_toast_for_version.is_some() {
            settings.pending_update_toast_for_version = None;
            settings.update(pool).await?;
        }
    }

    sqlx::query!(
        "
		INSERT INTO app_metadata (key, value, updated_at)
		VALUES ('app_version', ?, unixepoch())
		ON CONFLICT(key) DO UPDATE SET
			value = excluded.value,
			updated_at = excluded.updated_at
		",
        env!("CARGO_PKG_VERSION"),
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Cleans up data from the database that is no longer referenced, but must be
/// kept around for a little while to allow users to recover from accidental
/// deletions.
async fn stale_data_cleanup(pool: &Pool<Sqlite>) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

    let has_skin_tables = sqlx::query!(
		"SELECT COUNT(*) AS \"count!: i64\" FROM sqlite_master WHERE type = 'table' AND name IN ('custom_minecraft_skins', 'minecraft_users')",
	)
	.fetch_one(&mut *tx)
	.await?
	.count == 2;

    if has_skin_tables {
        sqlx::query!(
			"DELETE FROM custom_minecraft_skins WHERE minecraft_user_uuid NOT IN (SELECT uuid FROM minecraft_users)"
		)
		.execute(&mut *tx)
		.await?;
    }

    tx.commit().await?;

    Ok(())
}
