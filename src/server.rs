use log::{error, info};
use std::collections::{hash_map::Entry, HashMap};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use dotenv::dotenv;

mod messenger {
    tonic::include_proto!("messenger");
}

use messenger::{
    messenger_server::{Messenger, MessengerServer},
    Message, MessageResponse, MessengerRequest,
};

#[derive(Debug, Default)]
struct MessengerService {
    clients:
        Arc<Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Result<Message, Status>>>>>,
}

#[tonic::async_trait]
impl Messenger for MessengerService {
    async fn send_message(
        &self,
        request: Request<Message>,
    ) -> Result<Response<MessageResponse>, Status> {
        let message = request.into_inner();
        info!(
            "Got message {} from {} for {}",
            message.content, message.sender, message.recipient
        );
        let clients = self.clients.lock().await;
        if let Some(sender) = clients.get(&message.recipient) {
            sender
                .send(Ok(message))
                .inspect(|_| info!("The message was successfully delivered"))
                .map_err(|e| {
                    info!("Failed to deliver the message");
                    Status::internal(e.to_string())
                })?;
            Ok(Response::new(MessageResponse { success: true }))
        } else {
            error!("User {} not found", { message.recipient });
            Err(Status::not_found("User not found"))
        }
    }

    type ReceiveMessagesStream = UnboundedReceiverStream<Result<Message, Status>>;

    async fn receive_messages(
        &self,
        request: Request<MessengerRequest>,
    ) -> Result<Response<Self::ReceiveMessagesStream>, Status> {
        let nickname = request.into_inner().nickname;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        match self.clients.lock().await.entry(nickname.clone()) {
            Entry::Occupied(_) => Err(Status::already_exists(format!(
                "The user {} is already logged in",
                nickname
            ))),
            Entry::Vacant(e) => {
                info!("Registering new user: {}", nickname);
                e.insert(tx);
                Ok(Response::new(UnboundedReceiverStream::new(rx)))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    env_logger::init();
    let port = env::var("INTERNAL_GRPC_PORT")?;
    let addr = format!("127.0.0.1:{}", port).parse().unwrap();
    let messenger_service = MessengerService::default();

    Server::builder()
        .add_service(MessengerServer::new(messenger_service))
        .serve(addr)
        .await?;

    Ok(())
}
