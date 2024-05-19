use dotenv::dotenv;
use log::{error, info};
use prost::Message as ProstMessage;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client};
use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;
use std::env;
use tokio::sync::RwLock;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

mod messenger {
    tonic::include_proto!("messenger");
}

use messenger::{
    messenger_server::{Messenger, MessengerServer},
    Message, MessageResponse, MessengerRequest,
};

#[derive(Debug)]
struct MessengerService {
    clients:
        Arc<RwLock<HashMap<String, tokio::sync::mpsc::UnboundedSender<Result<Message, Status>>>>>,
    redis_conn: Arc<RwLock<MultiplexedConnection>>,
}

impl MessengerService {
    async fn new(redis_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::open(redis_url)?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        Ok(Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            redis_conn: Arc::new(RwLock::new(conn)),
        })
    }

    async fn store_message(&self, message: &Message) -> redis::RedisResult<()> {
        self.store_message_for(message, &message.recipient).await?;
        self.store_message_for(message, &message.sender).await?;
        Ok(())
    }

    async fn store_message_for(&self, message: &Message, user: &str) -> redis::RedisResult<()> {
        let key = format!("user:{}:messages", user);
        match self.redis_conn.try_write() {
            Ok(mut conn) => {
                conn.rpush(&key, &message.encode_to_vec()).await?;
                if message.delete_timestamp != 0 {
                    conn.expire_at(&key, message.delete_timestamp).await?;
                }
            }
            Err(_) => {
                let mut conn = self.redis_conn.read().await.clone();
                conn.rpush(&key, &message.encode_to_vec()).await?;
                if message.delete_timestamp != 0 {
                    conn.expire_at(&key, message.delete_timestamp).await?;
                }
            }
        };
        Ok(())
    }

    async fn get_message_history(&self, nickname: &str) -> redis::RedisResult<Vec<Message>> {
        let key = format!("user:{}:messages", nickname);
        let messages: Vec<Vec<u8>> = match self.redis_conn.try_write() {
            Ok(mut conn) => conn.lrange(key, 0, -1).await?,
            Err(_) => {
                self.redis_conn
                    .read()
                    .await
                    .clone()
                    .lrange(key, 0, -1)
                    .await?
            }
        };
        let messages: Vec<Message> = messages
            .into_iter()
            .filter_map(|s| Message::decode(s.as_slice()).ok())
            .collect();
        Ok(messages)
    }
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

        let clients = self.clients.read().await;
        if let Some(sender) = clients.get(&message.recipient) {
            sender
                .send(Ok(message.clone()))
                .inspect(|_| info!("The message was successfully delivered"))
                .map_err(|e| {
                    info!("Failed to deliver the message");
                    Status::internal(e.to_string())
                })?;
            drop(clients);
            self.store_message(&message).await.map_err(|e| {
                error!("Error storing message in Redis: {}", e);
                Status::internal("Failed to store message in Redis")
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
        let nickname = &request.get_ref().nickname;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut clients = self.clients.write().await;
        match clients.entry(nickname.clone()) {
            Entry::Occupied(e) => {
                if e.get().is_closed() {
                    info!("User {} logged out. Relogining...", nickname);
                    e.remove();
                    drop(clients);
                    self.receive_messages(request).await
                } else {
                    Err(Status::already_exists(format!(
                        "The user {} is already logged in",
                        nickname
                    )))
                }
            }
            Entry::Vacant(e) => {
                info!("Registering new user: {}", nickname);
                let tx = e.insert(tx);

                let history = self.get_message_history(&nickname).await.map_err(|e| {
                    error!("Error retrieving message history: {}", e);
                    Status::internal("Failed to retrieve message history")
                })?;

                for message in history {
                    if tx.send(Ok(message)).is_err() {
                        error!("Failed to send message history to {}", nickname);
                        break;
                    }
                }

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

    let redis_url = env::var("REDIS_URL").expect("REDIS_URL environment variable is not set");
    let messenger_service = MessengerService::new(&redis_url).await?;

    Server::builder()
        .add_service(MessengerServer::new(messenger_service))
        .serve(addr)
        .await?;

    Ok(())
}
