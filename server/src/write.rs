use std::fs::{create_dir_all, OpenOptions};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::{fs, io};

use std::io::prelude::*;

use axum::extract::multipart::{Field, MultipartError};
use chrono::{Local, NaiveTime};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep_until, Instant};
use tracing::{debug, error, info};

pub async fn schedule_data_backups(data_path: &Path, backup_path: &Path) {
    info!("Scheduling backups");
    loop {
        let backup_time = NaiveTime::from_hms_opt(2, 0, 0).unwrap();
        let now = Local::now().naive_local();
        let today = Local::now().date_naive();

        let next_run = if now.time() < backup_time {
            today.and_time(backup_time)
        } else {
            today.succ_opt().unwrap().and_time(backup_time)
        };

        let next_run_duration = next_run
            .and_utc()
            .signed_duration_since(Local::now().with_timezone(&chrono::Utc))
            .to_std()
            .unwrap();

        info!(
            "Next backup scheduled for: {} (in {:?})",
            next_run, next_run_duration
        );

        sleep_until(Instant::now() + next_run_duration).await;

        perform_backup(data_path, backup_path).await;
    }
}

async fn perform_backup(_data_path: &Path, _backup_path: &Path) {
    // TODO impl
    info!("Executing daily backup...");
}

fn map_to_io_error(e: MultipartError) -> Error {
    Error::new(ErrorKind::Other, e)
}

pub async fn write_all_chunks_of_field(path: &Path, mut field: Field<'_>) -> Result<usize, Error> {
    info!(
        "Trying to progressively write to {} - (content_type = {:?})",
        path.display(),
        field.content_type()
    );
    let mut file = File::create(path).await?;
    let mut chunk_counter = 0;
    let mut total_size_counter = 0;
    loop {
        match field.chunk().await {
            Err(e) => {
                error!("Error while chunking: {:?}", e);
                return Err(map_to_io_error(e));
            }
            Ok(option) => match option {
                None => {
                    info!(
                        "File written to {} ({})",
                        path.display(),
                        total_size_counter
                    );
                    break;
                }
                Some(b) => {
                    chunk_counter += 1;
                    let chunk_size = b.len();
                    total_size_counter += chunk_size;
                    debug!("{}: chunk-size = {}", chunk_counter, chunk_size);
                    file.write_all(&*b).await?;
                }
            },
        }
    }
    Ok(total_size_counter)
}

// TODO introduce switch flag to try both and measure mem-consumption and speed? would be interesting
pub async fn _write_all_at_once<'a>(path: &Path, field: Field<'a>) -> Result<(), Error> {
    info!(
        "Trying to write to {} - (content_type = {:?})",
        path.display(),
        field.content_type()
    );
    let result = field.bytes().await.map_err(map_to_io_error);

    if result.is_err() {
        let e = result.err().unwrap();
        error!("Error while getting bytes of field {}", e);
        return Err(e);
    };

    match fs::write(path, result?) {
        Ok(_) => {
            info!("File written to {}", path.display());
            Ok(())
        }
        Err(e) => {
            error!("Error while writing to {}: {}", path.display(), e);
            Err(e)
        }
    }
}

pub fn append_line(file_path: &Path, line: &str) {
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(file_path)
        .unwrap();

    if let Err(e) = writeln!(file, "{line}") {
        error!("Couldn't append to file: {}", e);
    }
}

pub fn create_all_paths_if_not_exist(paths: Vec<&Path>) -> io::Result<()> {
    for path in paths.into_iter() {
        if !path.exists() {
            create_dir_all(path)?
        }
    }
    Ok::<(), Error>(())
}

pub fn create_all_files_if_not_exist(paths: Vec<&Path>) -> io::Result<()> {
    for path in paths.into_iter() {
        if !path.exists() {
            fs::write(path, b"")?;
        }
    }
    Ok::<(), Error>(())
}
