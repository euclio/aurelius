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
extern crate handlebars_iron;
extern crate iron;
extern crate mount;
extern crate pulldown_cmark;
extern crate serde;
extern crate staticfile;
extern crate url;
extern crate websocket as websockets;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_json;

pub mod browser;
pub mod markdown;

mod http;
mod websocket;

use std::env;
use std::net::SocketAddr;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use websocket::Server as WebSocketServer;

/// The `Server` type constructs a new markdown preview server.
///
/// The server will listen for HTTP and WebSocket connections on arbitrary ports.
pub struct Server {
    config: Config,
}

/// A server that is listening for HTTP requests on a given port, and broadcasting rendered
/// markdown over a websocket on another port.
pub struct Handle {
    http_listening: http::Listening,
    websocket_addr: SocketAddr,
    markdown_sender: mpsc::Sender<String>,
}

/// Configuration for the markdown server.
#[derive(Debug, Clone)]
pub struct Config {
    /// The initial markdown to render when starting the server.
    pub initial_markdown: String,

    /// The syntax highlighting theme to use.
    ///
    /// Defaults to the github syntax highlighting theme.
    pub highlight_theme: String,

    /// The directory that static files should be served out of.
    ///
    /// Defaults to the current working directory.
    pub working_directory: PathBuf,

    /// Custom CSS that should be used to style the markdown.
    ///
    /// Defaults to the github styles.
    pub custom_css: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            working_directory: env::current_dir().unwrap().to_owned(),
            initial_markdown: "".to_owned(),
            highlight_theme: "github".to_owned(),
            custom_css: "/_static/vendor/github-markdown-css/github-markdown.css".to_owned(),
        }
    }
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
    /// Server::new_with_config(Config {
    ///     highlight_theme: "github".to_owned(), .. Default::default()
    /// });
    /// ```
    pub fn new_with_config(config: Config) -> Server {
        Server { config: config }
    }

    /// Starts the server.
    ///
    /// Returns a channel that can be used to send markdown to the server. The markdown will be
    /// sent as HTML to all clients of the websocket server.
    pub fn start(&mut self) -> Handle {
        let mut websocket_server = WebSocketServer::new(("localhost", 0));
        let websocket_sender = websocket_server.get_markdown_sender();
        let websocket_addr = websocket_server.local_addr().unwrap();

        let (markdown_sender, markdown_receiver) = mpsc::channel::<String>();

        thread::spawn(move || {
            websocket_server.start();
        });

        thread::spawn(move || {
            for markdown in markdown_receiver.iter() {
                let html: String = markdown::to_html(&markdown);
                websocket_sender.send(html);
            }
        });

        debug!("Starting http_server");
        let http_listening = http::Server::new(&self.config)
            .listen(("localhost", 0), websocket_addr.port()).unwrap();

        Handle {
            http_listening: http_listening,
            websocket_addr: websocket_addr,
            markdown_sender: markdown_sender,
        }
    }
}

impl Handle {
    /// Returns the socket address that the websocket server is listening on.
    pub fn websocket_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.websocket_addr)
    }

    /// Returns the socket address that the HTTP server is listening on.
    pub fn http_addr(&self) -> io::Result<SocketAddr> {
        self.http_listening.local_addr()
    }

    /// Changes the "current working directory" of the HTTP server. The HTTP server will serve
    /// static file requests out of the new directory.
    pub fn change_working_directory<P>(&mut self, dir: P)
        where P: AsRef<Path>
    {
        self.http_listening.change_working_directory(dir);
    }

    /// Publish new markdown to be rendered by the server.
    pub fn send<S>(&self, data: S)
        where S: Into<String>
    {
        self.markdown_sender.send(data.into()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::Server;

    #[test]
    fn sanity() {
        let mut server = Server::new();
        server.start();
    }
}
