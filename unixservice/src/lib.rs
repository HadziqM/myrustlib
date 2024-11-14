use bincode::{deserialize, serialize};
use log::{debug, error, warn};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    sync::Arc,
};

pub trait UnixServiceClient: Sized + Send + Sync + 'static {
    /// this type better serve as signal (enum)
    type Signal: Serialize + DeserializeOwned + Send + Sync + 'static;
    /// this type better serve as signal (enum)
    type Response: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// required to connect to socket name
    fn name() -> String;
    /// the self is reference counter so feel to use it
    fn handle_response(
        self: Arc<Self>,
        res: Self::Response,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn send_request(
        self: Arc<Self>,
        signal: Self::Signal,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let socket_path = Path::new("/tmp").join(format!("{}.sock", Self::name()));

        match UnixStream::connect(&socket_path) {
            Ok(mut stream) => {
                let msg = serialize(&signal)?;
                if let Err(e) = stream.write_all(&msg) {
                    error!("Error writing to stream: {}", e);
                    return Err(Box::new(e));
                }

                let mut buf = vec![];
                if let Err(e) = stream.read_to_end(&mut buf) {
                    error!("Error reading from stream: {}", e);
                    return Err(Box::new(e));
                }

                self.handle_response(deserialize(&buf)?)
            }
            Err(e) => {
                error!("Error connecting to socket: {}", e);
                Err(Box::new(e))
            }
        }
    }
}

pub trait UnixServiceServer: Sized + Sync + Send + 'static {
    /// this type better serve as signal (enum)
    type Signal: Serialize + DeserializeOwned + Send + Sync + 'static;
    /// this type better serve as signal (enum)
    type Response: Serialize + DeserializeOwned + Send + Sync + 'static;

    fn name() -> String {
        env!("CARGO_PKG_NAME").to_string()
    }
    /// the self is reference counter so feel to use it
    fn handle_request(
        self: Arc<Self>,
        signal: Self::Signal,
    ) -> Result<Self::Response, Box<dyn std::error::Error>>;

    fn create_service(self) -> Result<(), Box<dyn std::error::Error>> {
        let socket_path = Path::new("/tmp").join(format!("{}.sock", Self::name()));

        if socket_path.exists() {
            debug!("Removing old socket");
            if let Err(e) = std::fs::remove_file(&socket_path) {
                error!("Failed to remove old socket: {}", e);
                return Err(Box::new(e));
            }
        }

        let m = Arc::new(self);
        let listener = std::os::unix::net::UnixListener::bind(&socket_path)?;
        debug!("Listening on {:?}", socket_path);

        for request in listener.incoming() {
            match request {
                Ok(mut stream) => {
                    debug!("Received connection from {:?}", stream.peer_addr());
                    let mut buffer = vec![];
                    match stream.read_to_end(&mut buffer) {
                        Ok(_) => match deserialize::<Self::Signal>(&buffer) {
                            Ok(signal) => {
                                let mc = m.clone();
                                std::thread::spawn(move || {
                                    match Self::handle_request(mc, signal) {
                                        Ok(response) => {
                                            if let Ok(r) = serialize(&response) {
                                                if let Err(e) = stream.write_all(&r) {
                                                    error!("Failed to send response: {}", e);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error handling request: {}", e);
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Failed to deserialize signal: {}", e);
                            }
                        },
                        Err(e) => {
                            error!("Error reading data from stream: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Error accepting connection: {}", e);
                }
            }
        }

        Ok(())
    }
}
