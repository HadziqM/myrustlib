#![allow(async_fn_in_trait)]

use bincode::{deserialize, serialize};
use log::{debug, error};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    spawn,
};

pub trait TcpServiceClient: Sized + Send + Sync + 'static {
    /// this type better serve as signal (enum)
    type Signal: Serialize + DeserializeOwned + Send + Sync + 'static;
    /// this type better serve as signal (enum)
    type Response: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// required to connect to socket name
    fn address() -> String;

    /// send signal into server
    async fn send_request(
        self: Arc<Self>,
        signal: Self::Signal,
    ) -> Result<Self::Response, Box<dyn std::error::Error>> {
        match TcpStream::connect(Self::address()).await {
            Ok(mut stream) => {
                let msg = serialize(&signal)?;
                if let Err(e) = stream.write_all(&msg).await {
                    error!("Error writing to stream: {}", e);
                    return Err(Box::new(e));
                }

                let mut buf = vec![];
                if let Err(e) = stream.read_to_end(&mut buf).await {
                    error!("Error reading from stream: {}", e);
                    return Err(Box::new(e));
                }
                Ok(deserialize(&buf)?)
            }
            Err(e) => {
                error!("Error connecting to socket: {}", e);
                Err(Box::new(e))
            }
        }
    }
}

#[async_trait::async_trait]
pub trait TcpServiceServer: Sized + Sync + Send + 'static {
    /// Signal type for requests.
    type Signal: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Response type for responses.
    type Response: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Custom error type.
    type Error: std::error::Error + Send + Sync + 'static;

    fn address() -> String;

    /// Handle an incoming request.
    async fn handle_request(
        self: Arc<Self>,
        signal: Self::Signal,
    ) -> Result<Self::Response, Self::Error>;

    /// Create and run the TCP service.
    async fn create_service(self) -> Result<(), Box<dyn std::error::Error>> {
        let service = Arc::new(self);
        let listener = TcpListener::bind(Self::address()).await?;
        debug!("Listening on {}", Self::address());

        loop {
            let (mut socket, _) = listener.accept().await?;
            let service_clone = Arc::clone(&service);

            spawn(async move {
                let mut buf = vec![];
                match socket.read_to_end(&mut buf).await {
                    Ok(_) => match bincode::deserialize::<Self::Signal>(&buf) {
                        Ok(signal) => match service_clone.handle_request(signal).await {
                            Ok(response) => match bincode::serialize(&response) {
                                Ok(msg) => {
                                    if let Err(e) = socket.write_all(&msg).await {
                                        error!("Failed to write response: {}", e);
                                    }
                                }
                                Err(e) => error!("Serialization error: {}", e),
                            },
                            Err(e) => error!("Request handling error: {}", e),
                        },
                        Err(e) => error!("Deserialization error: {}", e),
                    },
                    Err(e) => error!("Socket read error: {}", e),
                }
            });
        }
    }
}
