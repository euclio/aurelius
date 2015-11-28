//! Contains the WebSocket server component.

use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;
use websockets::Server as WebSocketServer;
use websockets::{Message, Sender, Receiver};
use websockets::header::WebSocketProtocol;
use websockets::message::Type;

/// The WebSocket server.
///
/// Manages WebSocket connections from clients of the HTTP server.
pub struct Server {

    /// The port that the server is listening on.
    pub port: u16,
    active_connections: Arc<Mutex<HashMap<Uuid, mpsc::Sender<String>>>>,
}

impl Server {

    /// Creates a new server that listens on port `port`.
    pub fn new(port: u16) -> Server {
        Server {
            port: port,
            active_connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Starts the server.
    ///
    /// This method does not return.
    pub fn start(&self) {
        self.listen_forever()
    }

    /// Sends HTML data to all open WebSocket connections on the server.
    pub fn notify(&self, html: String) {
        for (uuid, sender) in self.active_connections.lock().unwrap().iter_mut() {
            debug!("notifying websocket {}", uuid);
            sender.send(html.to_owned()).unwrap();
        }
    }

    /// Listen for WebSocket connections.
    fn listen_forever(&self) {
        let server = WebSocketServer::bind(("localhost", self.port)).unwrap();
        info!("WebSockets listening on {}", self.port);

        for connection in server {
            let active_connections = self.active_connections.clone();

            // Spawn a new thread for each new connection.
            thread::spawn(move || {
                let request = connection.unwrap().read_request().unwrap();
                let headers = request.headers.clone();

                request.validate().unwrap();

                let mut response = request.accept();

                if let Some(&WebSocketProtocol(ref protocols)) = headers.get() {
                    if protocols.contains(&("rust-websocket".to_string())) {
                        response.headers.set(WebSocketProtocol(vec!["rust-websocket".to_string()]));
                    }
                }

                let mut client = response.send().unwrap();

                let ip = client.get_mut_sender()
                    .get_mut()
                    .peer_addr()
                    .unwrap();

                info!("Connection from {}", ip);

                // Create the send and recieve channdels for the websocket.
                let (mut sender, mut receiver) = client.split();

                // Create a two tranmitters from this channel so we can send messages from other
                // threads.
                // Created senders that will send markdown between threads
                let (md_tx, md_rx) = channel();
                let (message_tx, message_rx) = channel();
                let message_tx_2 = message_tx.clone();

                // Store the sender in the active connections.
                let uuid = Uuid::new_v4();
                active_connections.lock().unwrap().insert(uuid, md_tx);

                // Start a separate thread to manage sending messages to the client.
                thread::spawn(move || {
                    loop {
                        match md_rx.recv() {
                            Ok(m) => message_tx.send(Message::text(m)).unwrap(),
                            Err(e) => {
                                debug!("Send loop got error: {:?}", e);
                                return;
                            }
                        };
                    }
                });

                // Websocket send loop
                thread::spawn(move || {
                    loop {
                        let message: Message = match message_rx.recv() {
                            Ok(m) => m,
                            Err(e) => {
                                debug!("Send Loop: {:?}", e);
                                return;
                            }
                        };
                        match message.opcode {
                            Type::Close => {
                                let _ = sender.send_message(&message);
                                // If it's a close message, just send it and then return.
                                return;
                            },
                            _ => (),
                        }
                        // Send the message
                        match sender.send_message(&message) {
                            Ok(()) => (),
                            Err(e) => {
                                debug!("Send Loop: {:?}", e);
                                let _ = sender.send_message(&Message::close());
                                return;
                            }
                        }
                    }
                });

                // Websocket receive loop
                for message in receiver.incoming_messages() {
                    let message: Message = match message {
                        Ok(m) => m,
                        Err(e) => {
                            debug!("Receive loop got error: {}", e);
                            message_tx_2.send(Message::close()).unwrap();
                            return;
                        }
                    };
                    match message.opcode {
                        Type::Close => {
                            // The client has closed the connection.
                            message_tx_2.send(Message::close()).unwrap();
                            active_connections.lock().unwrap().remove(&uuid);
                            return;
                        },
                        Type::Ping => match message_tx_2.send(Message::pong(message.payload)) {
                            Ok(()) => (),
                            Err(e) => {
                                debug!("Receive Loop got error: {:?}", e);
                                return;
                            }
                        },
                        _ => () // Ignore all other messages.
                    }
                }
            });
        }
    }
}
