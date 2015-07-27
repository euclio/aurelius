//! [aurelius](https://github.com/euclio/aurelius) is a complete solution for rendering and
//! previewing markdown.
//!
//! This crate provides a server that can render and update an HTML preview of markdown without a
//! client-side refresh. The server listens for both WebSocket and HTTP connections on arbitrary
//! ports. Upon receiving an HTTP request, the server renders a page containing a markdown preview.
//! Client-side JavaScript then initiates a WebSocket connection which allows the server to push
//! changes to the client.
//!
//! This crate was designed to power [vim-markdown-composer], a markdown preview plugin for
//! [Neovim](http://neovim.io), but it may be used to implement similar plugins for any editor.
//! See [vim-markdown-composer] for a usage example.
//!
//! aurelius follows stable Rust. However, the API currently unstable and may change without
//! warning.
//!
//! # Acknowledgments
//! This crate is inspired by suan's
//! [instant-markdown-d](https://github.com/suan/instant-markdown-d).
//!
//! # Why the name?
//! "Aurelius" is a Roman *gens* (family name) shared by many famous Romans, including emperor
//! Marcus Aurelius, one of the "Five Good Emperors." The gens itself originates from the Latin
//! *aureus* meaning "golden." Also, tell me that "Markdown Aurelius" isn't a great pun.
//!
//! <cite>[Aurelia (gens) on Wikipedia](https://en.wikipedia.org/wiki/Aurelia_(gens))</cite>.
//!
//! [vim-markdown-composer]: https://github.com/euclio/vim-markdown-composer

extern crate hoedown;
extern crate porthole;
extern crate url;
extern crate uuid;
extern crate websocket as websockets;

#[macro_use] extern crate log;
#[macro_use] extern crate nickel;

pub mod browser;
pub mod markdown;

mod http;
mod websocket;

use http::Server as HttpServer;
use websocket::Server as WebSocketServer;

use std::sync::{Arc, RwLock};
use std::thread;

/// Representation of the running markdown server.
pub struct ServerHandle {
    server: Server,
}

impl ServerHandle {
    /// Returns the port that the WebSocket server is listening on.
    pub fn websocket_port(&self) -> u16 {
        let ws_server_lock = self.server.websocket_server.clone();
        let ws_server = ws_server_lock.read().unwrap();
        ws_server.port
    }

    /// Returns the port that the HTTP server is listening on.
    pub fn http_port(&self) -> u16 {
        let http_server_lock = self.server.http_server.clone();
        let http_server = http_server_lock.read().unwrap();
        http_server.port
    }

    /// Send a markdown string to be rendered by the server.
    ///
    /// The HTML will then be sent to all websocket connections.
    pub fn send_markdown(&self, markdown: String) {
        let ws_server_lock = self.server.websocket_server.clone();
        let ws_server = ws_server_lock.read().unwrap();
        ws_server.notify(markdown::to_html(&markdown));
    }
}

/// The `Server` type constructs a new markdown server.
///
/// The server will listen for HTTP and WebSocket connections on arbitrary ports.
pub struct Server {
    websocket_port: u16,
    http_server: Arc<RwLock<HttpServer>>,
    websocket_server: Arc<RwLock<WebSocketServer>>,
    initial_markdown: Option<String>,
}

impl Server {
    /// Creates a new markdown preview server.
    ///
    /// Builder methods are provided to configure the server before starting it.
    pub fn new() -> Server {
        let websocket_port = porthole::open().unwrap();
        let websocket_server = WebSocketServer::new(websocket_port);

        let http_port = porthole::open().unwrap();
        let http_server = HttpServer::new(http_port);

        Server {
            websocket_port: websocket_port,
            http_server: Arc::new(RwLock::new(http_server)),
            websocket_server: Arc::new(RwLock::new(websocket_server)),
            initial_markdown: None,
        }
    }

    /// Sets the markdown that the server should display when the first connection is received.
    pub fn initial_markdown(&mut self, markdown: &str) -> &mut Server {
        self.initial_markdown = Some(markdown.to_string());
        self
    }

    ///
    /// Starts the server, returning a `ServerHandle` to communicate with it.
    pub fn start(self) -> ServerHandle {
        let websocket_server = self.websocket_server.clone();

        // Start websocket server
        thread::spawn(move || {
            let server = websocket_server.read().unwrap();
            server.start();
        });

        let http_server = self.http_server.clone();
        let websocket_port = self.websocket_port;

        // Start http server
        let initial_markdown = match self.initial_markdown {
            Some(ref markdown) => markdown.clone(),
            None => "".to_string()
        };
        thread::spawn(move || {
            let server = http_server.read().unwrap();
            debug!("Starting http_server");
            server.start(websocket_port, initial_markdown);
        });

        ServerHandle { server: self }
    }
}
