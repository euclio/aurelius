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

#![deny(missing_docs)]

extern crate chan;
extern crate hoedown;
extern crate porthole;
extern crate url;
extern crate websocket as websockets;

#[macro_use]
extern crate log;
#[macro_use]
extern crate nickel;

pub mod browser;
pub mod markdown;

mod http;
mod websocket;

use std::net::SocketAddr;
use std::io;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::{self, Sender};
use std::thread;

use http::Server as HttpServer;
use websocket::Server as WebSocketServer;

/// The `Server` type constructs a new markdown preview server.
///
/// The server will listen for HTTP and WebSocket connections on arbitrary ports.
pub struct Server {
    http_server: Arc<RwLock<HttpServer>>,
    websocket_server: WebSocketServer,
    config: Config,
}

/// Configuration for the markdown server.
#[derive(Debug, Default, Clone)]
pub struct Config {
    /// The initial markdown to render when starting the server.
    pub initial_markdown: String,

    /// The syntax highlighting theme to use.
    pub highlight_theme: String,
}

impl Server {
    /// Creates a new markdown preview server.
    pub fn new() -> Server {
        Self::new_with_config(Config { ..Default::default() })
    }

    /// Creates a new configuration with the config struct.
    ///
    /// # Example
    /// ```
    /// use std::default::Default;
    /// use aurelius::{Config, Server};
    ///
    /// let server = Server::new_with_config(Config {
    ///     highlight_theme: "github".to_owned(), .. Default::default()
    /// });
    /// ```
    pub fn new_with_config(config: Config) -> Server {
        let http_port = porthole::open().unwrap();
        let http_server = HttpServer::new(http_port);

        Server {
            http_server: Arc::new(RwLock::new(http_server)),
            websocket_server: WebSocketServer::new(("localhost", 0)),
            config: config,
        }
    }

    /// Returns the socket address that the websocket server is listening on.
    pub fn websocket_addr(&self) -> io::Result<SocketAddr> {
        self.websocket_server.local_addr()
    }

    /// Starts the server, returning a `ServerHandle` to communicate with it.
    pub fn start(&mut self) -> Sender<String> {
        let (markdown_sender, markdown_receiver) = mpsc::channel::<String>();
        let websocket_sender = self.websocket_server.start();
        println!("websockets listening on {}", self.websocket_addr().unwrap());

        thread::spawn(move || {
            for markdown in markdown_receiver.iter() {
                let html: String = markdown::to_html(&markdown);
                println!("SENDING MARKDOWN: {}", html);
                websocket_sender.send(html);
            }
        });

        let http_server = self.http_server.clone();

        let websocket_port = self.websocket_server.local_addr().unwrap().port();

        let config = self.config.clone();
        thread::spawn(move || {
            let server = http_server.read().unwrap();
            debug!("Starting http_server");
            server.start(websocket_port,
                         config.initial_markdown,
                         config.highlight_theme);
        });

        markdown_sender
    }
}
