use std::env;

use dotenv::dotenv;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::StreamExt;

mod messenger {
    tonic::include_proto!("messenger");
}

use messenger::{messenger_client::MessengerClient, Message, MessengerRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let host = env::var("INTERNAL_GRPC_HOST")?;
    let port = env::var("INTERNAL_GRPC_PORT")?;
    let mut client = MessengerClient::connect(format!("http://{}:{}", host, port)).await?;


    let mut lines_reader = BufReader::new(tokio::io::stdin()).lines();
    let nickname;
    println!("Enter your username:");
    if let Some(nick) = lines_reader.next_line().await? {
        nickname = nick;
    } else {
        return Ok(());
    }
    println!("Got it");
    let mut inbound = client
        .receive_messages(MessengerRequest {
            nickname: nickname.clone(),
        })
        .await?
        .into_inner();

    tokio::spawn(async move {
        while let Some(message) = inbound.next().await {
            match message {
                Ok(message) => println!("From {}: {}", message.sender, message.content),
                Err(err) => println!("Error: {}", err),
            }
        }
    });
    println!("Enter your messages in format: '<username>: <message>'");
    while let Some(line) = lines_reader.next_line().await? {
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() == 2 {
            let recipient = parts[0].trim();
            let content = parts[1].trim();
            client
                .send_message(Message {
                    sender: nickname.clone(),
                    recipient: recipient.to_string(),
                    content: content.to_string(),
                    delete_timestamp: 0,
                })
                .await?;
        } else {
            println!("Incorrect syntax!")
        }
    }
    Ok(())
}
