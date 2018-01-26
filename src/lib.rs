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
#![recursion_limit = "1024"]

extern crate handlebars_iron;
extern crate iron;
extern crate mount;
extern crate pulldown_cmark;
extern crate serde;
extern crate staticfile;
extern crate url;
extern crate websocket as websockets;

#[macro_use]
extern crate chan;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_json;

pub mod browser;
pub mod errors;
pub mod markdown;

mod config;
mod http;
mod websocket;

use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::process::Command;

use config::ExternalRenderer;
use errors::*;

pub use config::Config;

/// An instance of the a markdown preview server.
///
/// The server will listen for HTTP and WebSocket connections on arbitrary ports.
///
/// # Examples
///
/// ```no_run
/// use aurelius::{Config, Server};
///
/// let listening = Server::new_with_config(
///     Config {
///         initial_markdown: Some(String::from("# Hello, world!")),
///         ..Default::default()
///     })
///     .start()
///     .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct Server {
    config: Config,
}

impl Server {
    /// Create a new markdown preview server.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new markdown preview server with the given configuration.
    pub fn new_with_config(config: Config) -> Self {
        Server { config }
    }

    /// Starts the server.
    ///
    /// Returns a channel that can be used to send markdown to the server. The markdown will be
    /// sent as HTML to all clients of the websocket server.
    pub fn start(&self) -> Result<Listening> {
        debug!("starting websocket server");
        let websocket_listening =
            websocket::Server::new().listen(("localhost", self.config.websocket_port))?;
        debug!(
            "websockets listening on {}",
            websocket_listening.local_addr()?
        );

        let config = &self.config;

        let initial_html = if let Some(ref initial_markdown) = config.initial_markdown.as_ref() {
            markdown_to_html(initial_markdown, &config.external_renderer)?
        } else {
            String::new()
        };

        debug!("starting http_server");
        let assigned_websocket_port = websocket_listening.local_addr()?.port();
        let http_listening = http::Server::new(
            self.config.working_directory.clone(),
            assigned_websocket_port,
            http::StyleConfig {
                css: config.custom_css.clone(),
                highlight_theme: config.highlight_theme.clone(),
            },
        ).listen(("localhost", config.http_port), &initial_html)?;
        debug!("http listening on {}", http_listening.local_addr()?);

        let listening = Listening {
            http_listening: http_listening,
            websocket_listening: websocket_listening,
            external_renderer: config.external_renderer.clone(),
        };

        Ok(listening)
    }
}

impl Default for Server {
    fn default() -> Self {
        Server {
            config: Default::default(),
        }
    }
}

/// A handle to an active preview server.
///
/// The server is listening for HTTP requests on a given port, and broadcasting rendered markdown
/// over a websocket connection on another port.
pub struct Listening {
    http_listening: http::Listening,
    websocket_listening: websocket::Listening,
    external_renderer: Option<ExternalRenderer>,
}

impl Listening {
    /// Returns the socket address that the websocket server is listening on.
    pub fn websocket_addr(&self) -> io::Result<SocketAddr> {
        self.websocket_listening.local_addr()
    }

    /// Returns the socket address that the HTTP server is listening on.
    pub fn http_addr(&self) -> io::Result<SocketAddr> {
        self.http_listening.local_addr()
    }

    /// Changes the "current working directory" of the HTTP server. The HTTP server will serve
    /// static file requests out of the new directory.
    pub fn change_working_directory<P>(&mut self, dir: P)
    where
        P: AsRef<Path>,
    {
        self.http_listening.change_working_directory(dir);
    }

    /// Publish new markdown to be rendered by the server.
    pub fn send(&self, markdown: &str) -> Result<()> {
        let html = markdown_to_html(markdown, &self.external_renderer)?;
        self.websocket_listening.send(html);
        Ok(())
    }
}

fn markdown_to_html(markdown: &str, external_command: &Option<ExternalRenderer>) -> Result<String> {
    let html = if let Some(&(ref command, ref args)) = external_command.as_ref() {
        let mut command = Command::new(command);
        command.args(args);
        markdown::to_html_external(command, markdown)?
    } else {
        markdown::to_html_cmark(markdown)
    };

    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::Server;

    #[test]
    fn sanity() {
        let server = Server::new();
        server.start().unwrap();
    }
}
