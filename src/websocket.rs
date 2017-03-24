//! Contains the WebSocket server component.

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::thread;

use chan;
use websockets::{Message, WebSocketStream};
use websockets::header::WebSocketProtocol;
use websockets::server::Request;

/// The WebSocket server.
///
/// Manages WebSocket connections from clients of the HTTP server.
pub struct Server {
    server: TcpListener,
    markdown_channel: (chan::Sender<String>, chan::Receiver<String>),
}

impl Server {
    /// Creates a new server that listens on port `port`.
    pub fn new<A>(socket_addr: A) -> Server
        where A: ToSocketAddrs
    {
        Server {
            server: TcpListener::bind(socket_addr).unwrap(),
            markdown_channel: chan::sync(0),
        }
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.server.local_addr()
    }

    pub fn get_markdown_sender(&self) -> chan::Sender<String> {
        self.markdown_channel.0.clone()
    }

    /// Starts the server.
    pub fn start(&self) {
        for connection in self.server.incoming() {
            let connection = connection.unwrap();
            let markdown_receiver = self.markdown_channel.1.clone();
            thread::spawn(move || {
                Self::handle_connection(connection, markdown_receiver);
            });
        }
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

        let mut client = response.send().unwrap();

        for markdown in &markdown_receiver {
            client.send_message(&Message::text(markdown)).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use websockets::{Client, Message, Receiver};
    use websockets::client::request::Url;

    #[test]
    fn initial_send() {
        let server = super::Server::new("localhost:0");
        let sender = server.get_markdown_sender();
        let server_port = server.local_addr().unwrap().port();

        thread::spawn(move || {
            server.start();
        });

        let url = Url::parse(&format!("ws://localhost:{}", server_port))
            .unwrap();

        let request = Client::connect(&url).unwrap();
        let response = request.send().unwrap();
        response.validate().unwrap();
        let (_, mut receiver) = response.begin().split();

        sender.send("Hello world!".to_owned());

        let message: Message = receiver.recv_message().unwrap();
        assert_eq!(String::from_utf8(message.payload.into_owned()).unwrap(),
                   "Hello world!");
    }

    #[test]
    fn multiple_send() {
        let server = super::Server::new("localhost:0");
        let sender = server.get_markdown_sender();
        let server_port = server.local_addr().unwrap().port();

        thread::spawn(move || {
            server.start();
        });

        let url = Url::parse(&format!("ws://localhost:{}", server_port)).unwrap();

        let request = Client::connect(&url).unwrap();
        let response = request.send().unwrap();
        response.validate().unwrap();
        let (_, mut receiver) = response.begin().split();
        let mut messages = receiver.incoming_messages();

        sender.send("Hello world!".to_owned());
        let hello_message: Message = messages.next().unwrap().unwrap();
        assert_eq!(String::from_utf8(hello_message.payload.into_owned()).unwrap(),
                   "Hello world!");

        sender.send("Goodbye world!".to_owned());
        let goodbye_message: Message = messages.next().unwrap().unwrap();
        assert_eq!(String::from_utf8(goodbye_message.payload.into_owned()).unwrap(),
                   "Goodbye world!");
    }
}
