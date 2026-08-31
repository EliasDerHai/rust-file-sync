use axum::extract::multipart::{Field, MultipartError};
use crc32fast::Hasher as Crc32Hasher;
use shared::content_hash::ContentHash;
use std::fs;
use std::io;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info};

fn map_to_io_error(e: MultipartError) -> io::Error {
    io::Error::other(e)
}

pub async fn write_all_chunks_of_field(
    path: &Path,
    mut field: Field<'_>,
) -> Result<(usize, ContentHash), io::Error> {
    info!(
        "Trying to progressively write to {} - (content_type = {:?})",
        path.display(),
        field.content_type()
    );
    let mut file = File::create(path).await?;
    let mut chunk_counter = 0;
    let mut total_size_counter = 0;
    let mut hasher = Crc32Hasher::new();
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
                Some(bytes) => {
                    chunk_counter += 1;
                    let chunk_size = bytes.len();
                    total_size_counter += chunk_size;
                    debug!("{}: chunk-size = {}", chunk_counter, chunk_size);
                    hasher.update(&bytes);
                    file.write_all(&bytes).await?;
                }
            },
        }
    }
    Ok((total_size_counter, ContentHash::from(hasher.finalize())))
}

// NOTE: introduce switch flag to try both and measure mem-consumption and speed? would be interesting
pub async fn _write_all_at_once(path: &Path, field: Field<'_>) -> Result<(), io::Error> {
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
