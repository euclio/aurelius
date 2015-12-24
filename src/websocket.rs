//! Contains the WebSocket server component.

use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
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

    /// Stores the last markdown received, so that we have something to send to new connections.
    last_markdown: Arc<RwLock<String>>,
}

impl Server {
    /// Creates a new server that listens on port `port`.
    pub fn new(port: u16) -> Server {
        Server {
            port: port,
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            last_markdown: Arc::new(RwLock::new(String::new())),
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
        let last_markdown_lock = self.last_markdown.clone();

        {
            let mut last_markdown = last_markdown_lock.write().unwrap();
            *last_markdown = html;
        }

        for (uuid, sender) in self.active_connections.lock().unwrap().iter_mut() {
            debug!("notifying websocket {}", uuid);
            sender.send(last_markdown_lock.read().unwrap().to_owned()).unwrap();
        }
    }

    /// Listen for WebSocket connections.
    fn listen_forever(&self) {
        let server = WebSocketServer::bind(("0.0.0.0", self.port)).unwrap();
        info!("WebSockets listening on {}", self.port);

        for connection in server {
            let active_connections = self.active_connections.clone();
            let last_markdown_lock = self.last_markdown.clone();

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

                let client = response.send().unwrap();

                // Create the send and recieve channdels for the websocket.
                let (mut sender, mut receiver) = client.split();

                // Create senders that will send markdown between threads.
                let (message_tx, message_rx) = channel();
                let (md_tx, md_rx) = channel();

                // Store the sender in the active connections.
                let uuid = Uuid::new_v4();
                active_connections.lock().unwrap().insert(uuid, md_tx.clone());

                let initial_markdown = last_markdown_lock.read().unwrap().to_owned();

                md_tx.send(initial_markdown).unwrap();

                // Message receiver
                let ws_message_tx = message_tx.clone();
                let receive_loop = thread::spawn(move || {
                    for message in receiver.incoming_messages() {
                        let message: Message = message.unwrap();

                        match message.opcode {
                            Type::Close => {
                                let message = Message::close();
                                ws_message_tx.send(message).unwrap();
                                return;
                            },
                            Type::Ping => {
                                let message = Message::pong(message.payload);
                                ws_message_tx.send(message).unwrap();
                            }
                            _ => ws_message_tx.send(message).unwrap(),
                        }
                    }
                });

                let send_loop = thread::spawn(move || {
                    for message in message_rx.recv() {
                        let message: Message = message;
                        sender.send_message(&message).unwrap();
                    }
                });

                for markdown in md_rx.recv() {
                    message_tx.send(Message::text(markdown)).unwrap();
                }

                let _ = send_loop.join();
                let _ = receive_loop.join();
            });
        }
    }
}
