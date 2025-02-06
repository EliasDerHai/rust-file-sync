use crate::config::read_config;
use futures_util::future::join_all;
use reqwest::multipart::Form;
use reqwest::Client;
use shared::file_event::FileEventType;
use shared::get_files_of_directory::{
    get_all_file_descriptions, get_file_description, FileDescription,
};
use shared::sync_instruction::SyncInstruction;
use std::path::{Path, PathBuf};
use task::spawn;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::{fs, task};

mod config;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<Vec<FileDescription>>(100);
    let dir_to_monitor = match read_config() {
        Err(error) => {
            panic!("Critical error: {:?}", error);
        }
        Ok(path) => path,
    };

    println!("Start monitoring changes in '{:?}'", dir_to_monitor);
    spawn(watch_directory(dir_to_monitor.clone(), tx));

    let mut last_scan: Option<Vec<FileDescription>> = None; // maybe persist this ? needed if files are deleted between sessions
    let mut last_deleted_files: Vec<FileDescription> = Vec::new();
    let client = Client::new();

    while let Some(scanned) = rx.recv().await {
        if let Some(last) = last_scan.clone() {
            last_deleted_files = determine_deleted_files(&last, &scanned);
            let futures = last_deleted_files
                .iter()
                .map(|deleted| {
                    client
                        .post("http://localhost:3000/delete")
                        .body(deleted.relative_path.to_serialized_string())
                        .send()
                })
                .collect::<Vec<_>>();
            let results = join_all(futures).await;
            let c = last_deleted_files.len();
            if c > 0 {
                println!(
                    "Sent {} delete events to server: [{}]",
                    c,
                    last_deleted_files
                        .iter()
                        .map(|desc| desc.file_name.clone())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
                results
                    .iter()
                    .for_each(|r| println!("Server replied: {:?}", r));
            }
        }

        match send_to_server_and_receive_instructions(&client, &scanned).await {
            Err(err) => println!("Error - failed to get instructions from server: {:?}", err),
            Ok(instructions) => {
                println!(
                    "{} Instructions received {:?}",
                    instructions.len(),
                    instructions
                );
                for instruction in instructions {
                    if let SyncInstruction::Download(ref path) = &instruction {
                        if last_deleted_files
                            .iter()
                            .any(|deleted| deleted.relative_path == *path)
                        {
                            // no need to follow the download instruction,
                            // because we now that this file was just deleted (breaking the loop)
                            continue;
                        }
                    }
                    execute(&client, instruction, dir_to_monitor.as_path())
                        .await
                        .unwrap();
                }
            }
        }

        last_scan = Some(scanned);
    }
}

async fn watch_directory(dir: PathBuf, tx: Sender<Vec<FileDescription>>) {
    loop {
        match get_all_file_descriptions(dir.as_path()) {
            Err(error) => eprintln!("Could not scan dir - {}", error),
            Ok(descriptions) => match tx.send(descriptions).await {
                Err(error) => eprintln!("Error while scanning {}", error),
                Ok(()) => println!("Scanned dir successfully"),
            },
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
            let file_path = p.resolve(root);

            match client
                .get("http://localhost:3000/download")
                .body(p.to_serialized_string())
                .send()
                .await
            {
                Ok(response) => {
                    println!("{:?}", response);
                    let file_content = response.bytes().await.map_err(|e| e.to_string())?;
                    match fs::write(file_path.as_path(), file_content).await {
                        Ok(()) => println!(
                            "Downloaded {} successfully",
                            file_path
                                .file_name()
                                .map(|osstr| osstr.to_string_lossy().to_string())
                                .unwrap_or("?".to_string())
                        ),
                        Err(e) => eprint!("Could not download file from server: {}", e),
                    }
                }
                Err(e) => eprint!("Could not download file from server: {}", e),
            }

            Ok(()) // todo proper returns
        }
        SyncInstruction::Delete(p) => {
            let p = &p.resolve(root);
            match tokio::fs::remove_file(&p).await {
                Err(err) => {
                    eprintln!("Could not follow delete instruction - {}", err);
                    Err(err.to_string())
                }
                Ok(()) => {
                    println!("Deleted {} successfully", &p.to_string_lossy());
                    Ok(())
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
/// Note: Deletes will reverb one time as server instructed deletes are again sent to the server as delete events.
/// This is due to the stateless nature of the client.
/// Since it doesn't do harm, it doesn't make sense to introduce state in order to deal with this reverb.
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
