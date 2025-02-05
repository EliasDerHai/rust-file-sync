use crate::config::read_config;
use futures_util::future::join_all;
use reqwest::multipart::Form;
use reqwest::{Client, Error, Response};
use shared::file_event::FileEventType;
use shared::get_files_of_directory::{
    get_all_file_descriptions, get_file_description, FileDescription,
};
use shared::sync_instruction::SyncInstruction;
use std::path::{Path, PathBuf};
use task::spawn;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::task;

mod config;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<Vec<FileDescription>>(100);
    let dir_to_monitor = match read_config() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Critical error: {:?}", error);
            return;
        }
    };

    println!("Start monitoring changes in '{:?}'", dir_to_monitor);
    spawn(watch_directory(dir_to_monitor.clone(), tx));

    let mut last_scan: Option<Vec<FileDescription>> = None; // maybe persist this ? needed if files are deleted between sessions
    let mut skip: Option<Vec<FileDescription>> = None;
    let client = Client::new();

    while let Some(scanned) = rx.recv().await {
        println!("Files scanned: {:?}", scanned);

        if let Some(last) = last_scan.clone() {
            let deleted_files = determine_deleted_files(&last, &scanned);
            println!("Files deleted: {:?}", deleted_files);
            let futures = deleted_files
                .clone()
                .into_iter()
                .map(|deleted| {
                    client
                        .post("http://localhost:3000/delete")
                        .body(deleted.relative_path.to_serialized_string())
                        .send()
                })
                .collect::<Vec<_>>();
            let c = futures.len();
            join_all(futures)
                .await
                .iter()
                .for_each(|r| println!("{:?}", r));
            if c > 0 {
                println!("Sent {} delete events to server", c);
            }
            skip = Some(deleted_files);
        }

        match send_to_server_and_receive_instructions(&client, &scanned).await {
            Ok(instructions) => {
                println!(
                    "{} Instructions received {:?} - skip: {:?}",
                    instructions.len(),
                    instructions,
                    skip
                );
                for instruction in instructions {
                    if let (SyncInstruction::Download(ref path), Some(ref skipped)) =
                        (&instruction, &skip)
                    {
                        if skipped.iter().any(|deleted| deleted.relative_path == *path) {
                            // no need to follow the upload instruction because we now that this file was just deleted (breaking the loop)
                            continue;
                        }
                    }
                    execute(&client, instruction, dir_to_monitor.as_path())
                        .await
                        .unwrap();
                }
            }
            Err(err) => println!("Error - failed to get instructions from server: {:?}", err),
        }

        last_scan = Some(scanned);
    }
}

async fn watch_directory(dir: PathBuf, tx: Sender<Vec<FileDescription>>) {
    loop {
        match get_all_file_descriptions(dir.as_path()) {
            Ok(descriptions) => match tx.send(descriptions).await {
                Ok(()) => println!("Scanned dir successfully"),
                Err(error) => eprintln!("Error while scanning {}", error),
            },
            Err(error) => eprintln!("Could not scan dir - {}", error),
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

async fn send_to_server_and_receive_instructions(
    client: &Client,
    scanned: &Vec<FileDescription>,
) -> Result<Vec<SyncInstruction>, reqwest::Error> {
    client
        .post("http://localhost:3000/sync")
        .json(scanned)
        .send()
        .await?
        .json::<Vec<SyncInstruction>>()
        .await
}

async fn execute(client: &Client, instruction: SyncInstruction, root: &Path) -> Result<(), String> {
    match instruction {
        SyncInstruction::Upload(p) => {
            let file_path = p.resolve(root);
            let description = get_file_description(file_path.as_path(), root)?;
            let relative_path_to_send = description.relative_path.get().join("/");
            let form: Form = Form::new()
                .text(
                    "utc_millis",
                    description.last_updated_utc_millis.to_string(),
                )
                .text("relative_path", relative_path_to_send)
                .text(
                    "event_type",
                    FileEventType::ChangeEvent.serialize_to_string(),
                )
                .file("file", file_path)
                .await
                .map_err(|e| e.to_string())?;

            let x = client
                .post("http://localhost:3000/upload")
                .multipart(form)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            println!(
                "Server responded with: {}",
                x.text().await.unwrap_or("No bueno...".to_string())
            );
            Ok(())
        }
        SyncInstruction::Download(p) => {
            todo!()
        }
        SyncInstruction::Delete(p) => {
            let p = &p.resolve(root);
            match tokio::fs::remove_file(&p).await {
                Ok(()) => {
                    println!("Deleted {} successfully", &p.to_string_lossy());
                    Ok(())
                }
                Err(err) => {
                    eprintln!("Could not follow delete instruction - {}", err);
                    Err(err.to_string())
                }
            }
        }
    }
}

/// on os level files are just there or not so we got to keep track of the last state
/// and diff it with the new one in order to determine file deletion and propagate the event accordingly
/// [`https://docs.rs/notify/latest/notify/index.html`] could also be an option for later, but
/// I prefer a more native approach for now
///
/// Note: Deletes will reverb once time as server instructed deletes are again sent to the server as delete events.
/// This is due to the stateless nature of the client. Since it doesn't do harm, it doesn't make sense to introduce state in order to deal with this reverb.
fn determine_deleted_files(
    last: &Vec<FileDescription>,
    curr: &Vec<FileDescription>,
) -> Vec<FileDescription> {
    last.into_iter()
        .filter(|prev_description| {
            !curr.iter().any(|curr_description| {
                prev_description.relative_path == curr_description.relative_path
            })
        })
        .map(|d| d.clone())
        .collect()
}
