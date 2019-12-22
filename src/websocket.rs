//! Contains the WebSocket server component.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::thread;

use crossbeam_channel::{self, select};
use websocket::OwnedMessage;
use websocket::sync::Server as WebSocketServer;

/// The WebSocket server.
///
/// Manages WebSocket connections from clients of the HTTP server.
pub struct Server {
    _private: (),
}

#[derive(Debug)]
pub struct Listening {
    addr: SocketAddr,
    html_sender: crossbeam_channel::Sender<String>,
}

impl Listening {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.addr)
    }

    pub fn send(&self, html: String) {
        let sender = &self.html_sender;

        select! {
            default => (),
            send(sender, html) -> _res => (),
        }
    }
}

impl Server {
    /// Creates a new server that listens on port `port`.
    pub fn new() -> Server {
        Server { _private: () }
    }

    /// Starts the server.
    pub fn listen<A>(self, addr: A) -> io::Result<Listening>
    where
        A: ToSocketAddrs,
    {
        let server = WebSocketServer::bind(addr)?;
        let addr = server.local_addr()?;

        let (html_sender, html_receiver) = crossbeam_channel::bounded(3);

        thread::spawn(move || {
            for connection in server.filter_map(Result::ok) {
                let receiver = html_receiver.clone();
                thread::spawn(move || {
                    let mut client = connection.accept().unwrap();

                    for html in &receiver {
                        client.send_message(&OwnedMessage::Text(html)).unwrap();
                    }
                });
            }
        });

        let listening = Listening {
            addr: addr,
            html_sender: html_sender,
        };

        Ok(listening)
    }
}

#[cfg(test)]
mod tests {
    use websocket::ClientBuilder;
    use websocket::client::Url;
    use websocket::ws::dataframe::DataFrame;

    #[test]
    fn initial_send() {
        let server = super::Server::new().listen("localhost:0").unwrap();
        let server_port = server.local_addr().unwrap().port();

        let url = Url::parse(&format!("ws://localhost:{}", server_port)).unwrap();

        let mut client = ClientBuilder::new(&url.as_str())
            .unwrap()
            .connect_insecure()
            .unwrap();

        server.send("<p>Hello world!</p>".to_string());

        let message = client.recv_message().unwrap();
        assert_eq!(
            String::from_utf8(message.take_payload()).unwrap(),
            "<p>Hello world!</p>"
        );
    }

    #[test]
    fn multiple_send() {
        let server = super::Server::new().listen("localhost:0").unwrap();
        let server_port = server.local_addr().unwrap().port();

        let url = Url::parse(&format!("ws://localhost:{}", server_port)).unwrap();

        let mut client = ClientBuilder::new(url.as_str())
            .unwrap()
            .connect_insecure()
            .unwrap();
        server.send("<p>Hello world!</p>".to_string());
        server.send("<p>Goodbye world!</p>".to_string());

        let hello_message = client.recv_message().unwrap();
        assert_eq!(
            String::from_utf8(hello_message.take_payload()).unwrap(),
            "<p>Hello world!</p>"
        );

        let goodbye_message = client.recv_message().unwrap();
        assert_eq!(
            String::from_utf8(goodbye_message.take_payload()).unwrap(),
            "<p>Goodbye world!</p>"
        );
    }
}
