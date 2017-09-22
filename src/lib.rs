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
extern crate shlex;
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

mod http;
mod websocket;

use std::env;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;

use shlex::Shlex;

use errors::*;

const DEFAULT_HIGHLIGHT_THEME: &'static str = "github";
const DEFAULT_CSS: &'static str = "/_static/vendor/github-markdown-css/github-markdown.css";

/// An instance of the a markdown preview server.
///
/// The server will listen for HTTP and WebSocket connections on arbitrary ports.
///
/// # Examples
///
/// ```no_run
/// use aurelius::Server;
///
/// let listening = Server::new()
///     .initial_markdown("<h1>Hello, world</h1>")
///     .start()
///     .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct Server {
    initial_markdown: String,
    working_directory: PathBuf,
    highlight_theme: String,
    css: String,
    websocket_port: u16,
    http_port: u16,
    external_renderer: Option<String>,
}

impl Server {
    /// Create a new markdown preview server.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the initial markdown to render when starting the server.
    pub fn initial_markdown<S: Into<String>>(&mut self, markdown: S) -> &Self {
        self.initial_markdown = markdown.into();
        self
    }

    /// Set the directory that static files should be served out of.
    ///
    /// Defaults to the process' current working directory.
    pub fn working_directory<D: Into<PathBuf>>(&mut self, directory: D) -> &Self {
        self.working_directory = directory.into();
        self
    }

    /// Set the syntax highlighting theme to use.
    ///
    /// Defaults to the "github" theme.
    pub fn highlight_theme<T: Into<String>>(&mut self, theme: T) -> &Self {
        self.highlight_theme = theme.into();
        self
    }

    /// Set the CSS that should be used to style the markdown.
    ///
    /// Defaults to github's CSS styles.
    pub fn css<C: Into<String>>(&mut self, css: C) -> &Self {
        self.css = css.into();
        self
    }

    /// Set the port to listen for websocket connections on.
    ///
    /// Defaults to an arbitrary port assigned by the OS.
    pub fn websocket_port(&mut self, port: u16) -> &Self {
        self.websocket_port = port;
        self
    }

    /// Set the port to listen for HTTP connections on.
    ///
    /// Defaults to an arbitrary port assigned by the OS.
    pub fn http_port(&mut self, port: u16) -> &Self {
        self.http_port = port;
        self
    }

    /// Set an external command to use instead of the in-memory markdown renderer.
    ///
    /// The command should read markdown from stdin, and output markdown on stdout.
    pub fn external_renderer<C: Into<String>>(&mut self, command: C) -> &Self {
        self.external_renderer = Some(command.into());
        self
    }

    /// Starts the server.
    ///
    /// Returns a channel that can be used to send markdown to the server. The markdown will be
    /// sent as HTML to all clients of the websocket server.
    pub fn start(&self) -> Result<Listening> {
        debug!("starting websocket server");
        let websocket_listening = websocket::Server::new().listen(
            ("localhost", self.websocket_port),
        )?;
        debug!(
            "websockets listening on {}",
            websocket_listening.local_addr()?
        );

        let initial_html = markdown_to_html(&self.initial_markdown, &self.external_renderer)?;

        debug!("starting http_server");
        let assigned_websocket_port = websocket_listening.local_addr()?.port();
        let http_listening = http::Server::new(
            self.working_directory.clone(),
            assigned_websocket_port,
            http::StyleConfig {
                css: self.css.clone(),
                highlight_theme: self.highlight_theme.clone(),
            },
        ).listen(("localhost", self.http_port), &initial_html)?;
        debug!("http listening on {}", http_listening.local_addr()?);

        let listening = Listening {
            http_listening: http_listening,
            websocket_listening: websocket_listening,
            external_renderer: self.external_renderer.clone(),
        };

        Ok(listening)
    }
}

impl Default for Server {
    fn default() -> Self {
        Server {
            working_directory: env::current_dir().unwrap().to_owned(),
            initial_markdown: String::default(),
            highlight_theme: DEFAULT_HIGHLIGHT_THEME.to_string(),
            css: DEFAULT_CSS.to_string(),
            websocket_port: 0,
            http_port: 0,
            external_renderer: None,
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
    external_renderer: Option<String>,
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
        self.websocket_listening.send(markdown_to_html(
            &markdown,
            &self.external_renderer,
        )?);

        Ok(())
    }
}

fn markdown_to_html(markdown: &str, external_command: &Option<String>) -> Result<String> {
    let html = if let Some(ref command) = *external_command {
        let mut shlex = Shlex::new(command);

        let renderer = shlex.next().ok_or_else(|| "no external renderer specified")?;
        let mut command = Command::new(renderer);
        command.args(shlex);
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
