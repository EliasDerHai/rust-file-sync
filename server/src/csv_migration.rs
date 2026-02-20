use crate::db::ServerDatabase;
use crate::file_event::FileEvent;
use std::collections::HashMap;
use std::path::Path;
use tracing::{error, info, warn};

/// One-time migration: reads `data/history.csv`, inserts rows into `file_event` table,
/// then renames the CSV to `history.csv.migrated`.
pub async fn migrate_csv_history_to_db(db: &ServerDatabase) {
    let csv_path = super::HISTORY_CSV_PATH.iter().as_path();
    if !csv_path.exists() {
        return;
    }

    let content = match std::fs::read_to_string(csv_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not read history.csv for migration: {e}");
            return;
        }
    };

    let events: Vec<FileEvent> = content
        .lines()
        .skip(1) // skip CSV header
        .filter_map(|line| {
            FileEvent::try_from(line)
                .map_err(|e| warn!("Skipping CSV line during migration: {e}"))
                .ok()
        })
        .collect();

    if events.is_empty() {
        info!("history.csv is empty — skipping migration, renaming file");
        rename_csv_after_migration(csv_path);
        return;
    }

    // Build hostname → client_id map from DB
    let hostname_to_client_id: HashMap<String, String> =
        match sqlx::query!(r#"SELECT id, host_name FROM client ORDER BY created_at ASC"#)
            .fetch_all(db.file_event().pool())
            .await
        {
            Ok(rows) => rows.into_iter().map(|r| (r.host_name, r.id)).collect(),
            Err(e) => {
                warn!("Could not query clients for CSV migration: {e}");
                HashMap::new()
            }
        };

    // Fallback client_id: oldest client in DB
    let fallback_client_id =
        match sqlx::query_scalar!(r#"SELECT id FROM client ORDER BY created_at ASC LIMIT 1"#)
            .fetch_optional(db.file_event().pool())
            .await
        {
            Ok(Some(id)) => id,
            _ => {
                warn!("No clients in DB — cannot migrate CSV history (no client_id to assign)");
                return;
            }
        };

    let mut unmapped_hosts = 0u64;
    let mapped_events: Vec<(FileEvent, String)> = events
        .into_iter()
        .map(|event| {
            let client_id = event
                .client_host
                .as_ref()
                .and_then(|host| hostname_to_client_id.get(host))
                .cloned()
                .unwrap_or_else(|| {
                    unmapped_hosts += 1;
                    fallback_client_id.clone()
                });
            (event, client_id)
        })
        .collect();

    let count = mapped_events.len();
    match db.file_event().bulk_insert(mapped_events).await {
        Ok(inserted) => {
            info!(
                "CSV migration complete: {inserted}/{count} events inserted ({unmapped_hosts} used fallback client)"
            );
            rename_csv_after_migration(csv_path);
        }
        Err(e) => {
            error!("CSV migration failed: {e}");
        }
    }
}

fn rename_csv_after_migration(csv_path: &Path) {
    let migrated_path = csv_path.with_extension("csv.migrated");
    if let Err(e) = std::fs::rename(csv_path, &migrated_path) {
        warn!("Could not rename history.csv to history.csv.migrated: {e}");
    } else {
        info!("Renamed history.csv → history.csv.migrated");
    }
}
