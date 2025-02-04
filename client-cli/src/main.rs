use crate::config::read_config;
use reqwest::{Client, RequestBuilder};
use shared::get_files_of_directory::{get_files_of_dir_rec, FileDescription};
use shared::sync_instruction::SyncInstruction;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

mod config;

#[tokio::main]
async fn main() {
    let p = read_config();
    let (tx, mut rx) = mpsc::channel::<Vec<FileDescription>>(100);

    match p {
        Ok(target) => {
            println!("Start monitoring changes in '{:?}'", target);
            tokio::spawn(watch_directory(target, tx));
        }
        Err(err) => {
            eprintln!("Critical error: {:?}", err);
            return;
        }
    }

    let client = Client::new();
    while let Some(scanned) = rx.recv().await {
        println!("Files scanned: {:?}", scanned.len());
        match send_to_server_and_receive_instructions(&client, &scanned).await {
            Ok(instructions) => {
                println!("Instructions: {:?}", instructions);
                todo!("add executing instructions")
            }
            Err(err) => println!("Error: {:?}", err),
        }
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

async fn watch_directory(dir: PathBuf, tx: Sender<Vec<FileDescription>>) {
    loop {
        match get_files_of_dir_rec(dir.as_path()) {
            Ok(descriptions) => match tx.send(descriptions).await {
                Ok(()) => println!("Scanned dir successfully"),
                Err(error) => eprintln!("Error while scanning {}", error),
            },
            Err(error) => eprintln!("Could not scan dir - {}", error),
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
