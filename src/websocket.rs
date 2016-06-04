//! Contains the WebSocket server component.

use std::io;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::ops::Deref;
use std::sync::mpsc::channel;
use std::thread;

use chan;
use websockets::{Message, Sender, Receiver, WebSocketStream};
use websockets::header::WebSocketProtocol;
use websockets::message::Type;
use websockets::server::Request;
use websockets::result::WebSocketError;

/// The WebSocket server.
///
/// Manages WebSocket connections from clients of the HTTP server.
pub struct Server {
    listener: TcpListener,
    html_sender: chan::Sender<String>,
    html_receiver: chan::Receiver<String>,
}

impl Server {
    /// Creates a new server that listens on port `port`.
    pub fn bind<A>(socket_addr: A) -> io::Result<Server>
        where A: ToSocketAddrs
    {
        let (tx, rx) = chan::sync(0);

        let server = Server {
            listener: try!(TcpListener::bind(socket_addr)),
            html_sender: tx,
            html_receiver: rx,
        };

        let html_receiver = server.html_receiver.clone();
        let listener = server.listener.try_clone().unwrap();
        thread::spawn(move || {
            for connection in listener.incoming() {
                let connection = connection.unwrap();
                let html_receiver = html_receiver.clone();
                thread::spawn(move || {
                    Self::handle_connection(connection, html_receiver);
                });
            }
        });

        Ok(server)
    }

    pub fn send(&self, data: &str) {
        self.html_sender.send(String::from(data));
    }

    fn handle_connection(connection: TcpStream, markdown_receiver: chan::Receiver<String>) {
        let stream = WebSocketStream::Tcp(connection);
        let request = Request::read(stream.try_clone().unwrap(), stream.try_clone().unwrap())
            .unwrap();
        let headers = request.headers.clone();

        request.validate().unwrap();

        let mut response = request.accept();

        if let Some(&WebSocketProtocol(ref protocols)) = headers.get() {
            if protocols.contains(&("rust-websocket".to_string())) {
                response.headers.set(WebSocketProtocol(vec!["rust-websocket".to_string()]));
            }
        }

        let client = response.send().unwrap();

        // Create the send and receive channels for the websocket.
        let (mut sender, mut receiver) = client.split();

        // Create senders that will send websocket messages between threads.
        let (message_tx, message_rx) = channel();

        // Message receiver
        let ws_message_tx = message_tx.clone();
        let _ = thread::Builder::new()
            .name("ws_receive_loop".to_owned())
            .spawn(move || {
                for message in receiver.incoming_messages() {
                    let message: Message = match message {
                        Ok(m) => m,
                        Err(_) => {
                            let _ = ws_message_tx.send(Message::close());
                            return;
                        }
                    };

                    match message.opcode {
                        Type::Close => {
                            let message = Message::close();
                            ws_message_tx.send(message).unwrap();
                            return;
                        }
                        Type::Ping => {
                            let message = Message::pong(message.payload);
                            ws_message_tx.send(message).unwrap();
                        }
                        _ => ws_message_tx.send(message).unwrap(),
                    }
                }
            })
            .unwrap();

        let _ = thread::Builder::new()
            .name("ws_send_loop".to_owned())
            .spawn(move || {
                for message in message_rx.iter() {
                    let message: Message = message;
                    sender.send_message(&message)
                        .or_else(|e| {
                            match e {
                                WebSocketError::IoError(e) => {
                                    match e.kind() {
                                        io::ErrorKind::BrokenPipe => Ok(()),
                                        _ => Err(e),
                                    }
                                }
                                _ => panic!(e),
                            }
                        })
                        .unwrap();
                }
            })
            .unwrap();

        for markdown in markdown_receiver.iter() {
            message_tx.send(Message::text(markdown)).unwrap();
        }
    }
}

impl Deref for Server {
    type Target = TcpListener;
    fn deref(&self) -> &Self::Target {
        &self.listener
    }
}

#[cfg(test)]
mod tests {
    use std::io::prelude::*;

    use websockets::{Client, Message, Receiver};
    use websockets::client::request::Url;

    use super::Server;

    #[test]
    fn initial_send() {
        let server = Server::bind("localhost:0").unwrap();
        let url = Url::parse(&format!("ws://localhost:{}", server.local_addr().unwrap().port()))
            .unwrap();

        let request = Client::connect(&url).unwrap();
        let response = request.send().unwrap();
        response.validate().unwrap();
        let (_, mut receiver) = response.begin().split();

        server.send("Hello world!");

        let message: Message = receiver.recv_message().unwrap();
        assert_eq!(String::from_utf8(message.payload.into_owned()).unwrap(),
                   "Hello world!");
    }

    #[test]
    fn multiple_send() {
        let server = Server::bind("localhost:0").unwrap();
        let url = Url::parse(&format!("ws://localhost:{}", server.local_addr().unwrap().port()))
            .unwrap();

        let request = Client::connect(&url).unwrap();
        let response = request.send().unwrap();
        response.validate().unwrap();
        let (_, mut receiver) = response.begin().split();
        let mut messages = receiver.incoming_messages();

        server.send("Hello world!");
        let hello_message: Message = messages.next().unwrap().unwrap();
        assert_eq!(String::from_utf8(hello_message.payload.into_owned()).unwrap(),
                   "Hello world!");

        server.send("Goodbye world!");
        let goodbye_message: Message = messages.next().unwrap().unwrap();
        assert_eq!(String::from_utf8(goodbye_message.payload.into_owned()).unwrap(),
                   "Goodbye world!");
    }
}
