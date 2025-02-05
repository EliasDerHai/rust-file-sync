use crate::config::read_config;
use reqwest::multipart::Form;
use reqwest::Client;
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

    let client = Client::new();
    while let Some(scanned) = rx.recv().await {
        println!("Files scanned: {:?}", scanned);
        match send_to_server_and_receive_instructions(&client, &scanned).await {
            Ok(instructions) => {
                println!("{} Instructions received {:?}", instructions.len(), instructions);
                for instruction in instructions {
                    execute(&client, instruction, dir_to_monitor.as_path())
                        .await
                        .unwrap();
                }
            }
            Err(err) => println!("Error - failed to get instructions from server: {:?}", err),
        }
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
                .text("utc_millis", description.last_updated_utc_millis.to_string())
                .text(
                    "relative_path",
                    relative_path_to_send,
                )
                .text("event_type", "create".to_string()) // todo fix later
                .file("file", file_path)
                .await
                .map_err(|e| e.to_string())?;

            let x = client
                .post("http://localhost:3000/upload")
                .multipart(form)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            println!("{:?}", x.text().await.unwrap_or("No bueno...".to_string()));
            Ok(())
        }
        SyncInstruction::Download(p) => {
            todo!()
        }
        SyncInstruction::Delete(p) => {
            todo!()
        }
    }
}
