//! Contains the WebSocket server component.

use std::io;
use std::mem;
use std::net::{SocketAddr, ToSocketAddrs};
use std::thread;

use chan;
use websockets::{Message, Server as WebSocketServer};
use websockets::server::NoSslAcceptor;

/// The WebSocket server.
///
/// Manages WebSocket connections from clients of the HTTP server.
pub struct Server {
    server: Option<WebSocketServer<NoSslAcceptor>>,
    markdown_channel: (chan::Sender<String>, chan::Receiver<String>),
    local_addr: SocketAddr,
}

impl Server {
    /// Creates a new server that listens on port `port`.
    pub fn new<A>(socket_addr: A) -> Server
        where A: ToSocketAddrs
    {
        let server = WebSocketServer::bind(socket_addr).unwrap();
        let local_addr = server.local_addr().unwrap();

        Server {
            server: Some(server),
            markdown_channel: chan::sync(0),
            local_addr: local_addr,
        }
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local_addr)
    }

    pub fn get_markdown_sender(&self) -> chan::Sender<String> {
        self.markdown_channel.0.clone()
    }

    /// Starts the server.
    pub fn start(&mut self) {
        let server = mem::replace(&mut self.server, None);

        for connection in server.unwrap().filter_map(Result::ok) {
            let markdown_receiver = self.markdown_channel.1.clone();
            thread::spawn(move || {
                let mut client = connection.accept().unwrap();

                for markdown in &markdown_receiver {
                    client.send_message(&Message::text(markdown)).unwrap();
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use websockets::{ClientBuilder, Message};
    use websockets::client::Url;

    #[test]
    fn initial_send() {
        let mut server = super::Server::new("localhost:0");
        let sender = server.get_markdown_sender();
        let server_port = server.local_addr().unwrap().port();

        thread::spawn(move || {
            server.start();
        });

        let url = Url::parse(&format!("ws://localhost:{}", server_port))
            .unwrap();

        let mut client = ClientBuilder::new(&url.as_str()).unwrap().connect_insecure().unwrap();

        sender.send("Hello world!".to_string());

        let message: Message = client.recv_message().unwrap();
        assert_eq!(String::from_utf8(message.payload.to_vec()).unwrap(), "Hello world!");
    }

    #[test]
    fn multiple_send() {
        let mut server = super::Server::new("localhost:0");
        let sender = server.get_markdown_sender();
        let server_port = server.local_addr().unwrap().port();

        thread::spawn(move || {
            server.start();
        });

        let url = Url::parse(&format!("ws://localhost:{}", server_port)).unwrap();

        let mut client = ClientBuilder::new(url.as_str()).unwrap().connect_insecure().unwrap();
        sender.send("Hello world!".to_string());
        sender.send("Goodbye world!".to_string());

        let hello_message: Message = client.recv_message().unwrap();
        assert_eq!(String::from_utf8(hello_message.payload.to_vec()).unwrap(), "Hello world!");

        let goodbye_message: Message = client.recv_message().unwrap();
        assert_eq!(String::from_utf8(goodbye_message.payload.to_vec()).unwrap(), "Goodbye world!");
    }
}
