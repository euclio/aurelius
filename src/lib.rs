//! [aurelius](https://github.com/euclio/aurelius) is a complete solution for live-previewing
//! markdown (and more!) as HTML.
//!
//! This crate provides a [`Server`] that can render and update an HTML preview of input without a
//! client-side refresh. Upon receiving an HTTP request, the server responds with an HTML page
//! containing a rendering of the input. Client-side JavaScript then initiates a WebSocket
//! connection which allows the server to push changes to the client.
//!
//! This crate was designed to power [vim-markdown-composer], a markdown preview plugin for
//! [Neovim](http://neovim.io), but it may be used to implement similar plugins for any editor. It
//! also supports arbitrary renderers through the [`Renderer`] trait.
//! See [vim-markdown-composer] for a real-world usage example.
//!
//! # Example
//!
//! ```no_run
//! use std::net::SocketAddr;
//! use aurelius::Server;
//! use aurelius::render::MarkdownRenderer;
//!
//! # tokio_test::block_on(async {
//! let addr = "127.0.0.1:1337".parse::<SocketAddr>()?;
//! let mut server = Server::bind(&addr, MarkdownRenderer::new()).await?;
//!
//! server.open_browser()?;
//!
//! server.send("# Hello, world!");
//! #   Ok::<_, Box<dyn std::error::Error>>(())
//! # });
//! ```
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

#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, RwLock};

use axum::{extract::Extension, http::Uri, routing::get, Router};
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio::sync::watch::{self, Sender};
use tower_http::trace::TraceLayer;
use tracing::log::*;

pub mod render;
mod service;

use crate::render::Renderer;

/// Live preview server.
///
/// Listens for HTTP connections and serves a page containing a live rendered preview. The page
/// contains JavaScript to open a websocket connection back to the server for rendering updates.
///
/// The server is asynchronous, and assumes that a `tokio` runtime is in use.
pub struct Server {
    addr: SocketAddr,
    config: Arc<RwLock<Config>>,
    renderer: Box<dyn Renderer>,
    output: RefCell<String>,
    tx: Sender<String>,
    _shutdown_tx: oneshot::Sender<()>,
}

impl Server {
    /// Binds the server to a specified address `addr` using the provided `renderer`.
    ///
    /// Binding to port 0 will request a port assignment from the OS. Use [`addr()`][Self::addr]
    /// to determine what port was assigned.
    ///
    /// The server must be bound using a Tokio runtime.
    pub async fn bind<R>(addr: &SocketAddr, renderer: R) -> io::Result<Server>
    where
        R: Renderer + 'static,
    {
        let (tx, rx) = watch::channel(String::new());
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let config = Arc::default();

        let app = Router::new()
            .route("/", get(service::websocket_handler))
            .route("/__/*path", get(service::serve_asset))
            .fallback(get(service::serve_static_file))
            .layer(Extension(Arc::clone(&config)))
            .layer(Extension(rx))
            .layer(TraceLayer::new_for_http());

        let http_server = axum::Server::bind(addr).serve(app.into_make_service());

        let addr = http_server.local_addr();
        info!("listening on {:?}", addr);

        let http_server = http_server.with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });

        tokio::spawn(http_server);

        Ok(Server {
            addr,
            config,
            renderer: Box::new(renderer),
            tx,
            output: RefCell::new(String::new()),
            _shutdown_tx: shutdown_tx,
        })
    }

    /// Returns the socket address that the server is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Publish new input to be rendered by the server.
    ///
    /// The new HTML will be sent to all connected websocket clients.
    pub async fn send(&self, input: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut output = self.output.take();
        output.clear();

        // Heuristic taken from rustdoc
        output.reserve(input.len() * 3 / 2);

        self.renderer.render(input, &mut output)?;

        self.output.replace(self.tx.send_replace(output));

        Ok(())
    }

    /// Set the directory that static files will be served from.
    ///
    /// This can be thought of as the "working directory" of the server. Any HTTP requests with
    /// non-root paths will be joined to this folder and used to serve files from the filesystem.
    /// Typically this is used to serve image links relative to the input file.
    ///
    /// By default, the server will not serve static files.
    pub fn set_static_root(&mut self, root: impl Into<PathBuf>) {
        self.config.write().unwrap().static_root = Some(root.into());
    }

    /// Set the highlight.js theme used for code blocks.
    ///
    /// Defaults to "github".
    pub fn set_highlight_theme(&mut self, theme: String) {
        self.config.write().unwrap().highlight_theme = theme;
    }

    /// Set custom CSS links and files to be served with the rendered HTML.
    ///
    /// Accepts URLs and absolute paths. URLs will be inserted as `<link>` tags. The contents of
    /// the paths will be read from disk and served in `<style>` tags.
    pub fn set_custom_css(&mut self, stylesheets: Vec<String>) -> io::Result<()> {
        let mut files = vec![];
        let mut links = vec![];

        for stylesheet in &stylesheets {
            // NB: Absolute paths on Windows will parse as URLs.
            match stylesheet.parse::<Uri>() {
                Ok(url)
                    if url.scheme_str() == Some("http") || url.scheme_str() == Some("https") =>
                {
                    links.push(url)
                }
                _ => files.push(Path::new(stylesheet.trim_start_matches("file://"))),
            }
        }

        let mut config = self.config.write().unwrap();

        config.custom_styles = files
            .into_iter()
            .map(fs::read_to_string)
            .collect::<Result<Vec<_>, _>>()?;
        config.css_links = links;

        Ok(())
    }

    /// Opens the user's default browser with the server's URL in the background.
    ///
    /// This function uses platform-specific utilities to determine the browser. The following
    /// platforms are supported:
    ///
    /// | Platform | Program    |
    /// | -------- | ---------- |
    /// | Linux    | `xdg-open` |
    /// | OS X     | `open -g`  |
    /// | Windows  | `explorer` |
    pub fn open_browser(&self) -> io::Result<()> {
        let command = if cfg!(target_os = "macos") {
            let mut command = Command::new("open");
            command.arg("-g");
            command
        } else if cfg!(target_os = "windows") {
            Command::new("explorer")
        } else {
            Command::new("xdg-open")
        };

        self.open_specific_browser(command)
    }

    /// Opens a browser with a specified command. The HTTP address of the server will be appended
    /// to the command as an argument.
    pub fn open_specific_browser(&self, mut command: Command) -> io::Result<()> {
        command.arg(&format!("http://{}", self.addr()));

        command.stdout(Stdio::null()).stderr(Stdio::null());

        info!("spawning browser: {:?}", command);
        command.spawn()?;
        Ok(())
    }
}

impl fmt::Debug for Server {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Server")
            .field("addr", &self.addr)
            .field("config", &self.config)
            .field("renderer", &"(dyn Renderer)")
            .field("output", &self.output)
            .field("tx", &self.tx)
            .field("_shutdown_tx", &self._shutdown_tx)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct Config {
    static_root: Option<PathBuf>,
    highlight_theme: String,
    css_links: Vec<Uri>,
    custom_styles: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            static_root: None,
            highlight_theme: String::from("github"),
            css_links: vec![],
            custom_styles: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use async_tungstenite::tungstenite::{self, error::ProtocolError, Message};
    use async_tungstenite::WebSocketStream;
    use futures::{AsyncRead, AsyncWrite};
    use futures::{SinkExt, StreamExt, TryStreamExt};
    use matches::assert_matches;
    use tokio::net::lookup_host;
    use tokio::time::{timeout, Duration};

    use crate::render::MarkdownRenderer;
    use crate::Server;

    async fn new_server() -> anyhow::Result<Server> {
        let addr = lookup_host("localhost:0").await?.next().unwrap();
        Ok(Server::bind(&addr, MarkdownRenderer::new()).await?)
    }

    async fn assert_websocket_closed<S: AsyncRead + AsyncWrite + Unpin>(
        websocket: &mut WebSocketStream<S>,
    ) {
        assert_matches!(
            websocket.send(Message::Text(String::new())).await,
            Err(tungstenite::Error::AlreadyClosed
                | tungstenite::Error::Protocol(ProtocolError::SendAfterClosing))
        );
    }

    #[tokio::test]
    async fn connect_http() -> anyhow::Result<()> {
        let server = new_server().await?;

        let res = reqwest::get(&format!("http://{}", server.addr())).await?;

        assert!(res.headers()["Content-Type"]
            .to_str()
            .unwrap()
            .contains("text/html"));

        let body = res.text().await?;

        assert!(body.contains("<html>"));

        Ok(())
    }

    #[tokio::test]
    async fn connect_websocket() -> anyhow::Result<()> {
        let server = new_server().await?;

        async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        Ok(())
    }

    #[tokio::test]
    async fn send_with_no_clients() -> Result<(), Box<dyn Error + Send + Sync>> {
        let server = new_server().await?;

        server.send("This shouldn't hang").await?;

        Ok(())
    }

    #[tokio::test]
    async fn send_html() -> anyhow::Result<()> {
        let server = new_server().await.unwrap();

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr()))
                .await
                .unwrap();

        server.send("<p>Hello, world!</p>").await.unwrap();
        let message = websocket.next().await.unwrap().unwrap();
        assert_eq!(message.to_text().unwrap(), "<p>Hello, world!</p>");

        server.send("<p>Goodbye, world!</p>").await.unwrap();
        let message = websocket.next().await.unwrap().unwrap();
        assert_eq!(message.to_text().unwrap(), "<p>Goodbye, world!</p>");

        Ok(())
    }

    #[tokio::test]
    async fn send_markdown() -> Result<(), Box<dyn Error + Send + Sync>> {
        let server = new_server().await?;

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        server.send("*Hello*").await?;
        let message = websocket.next().await.unwrap()?;
        assert_eq!(message.to_text()?.trim(), "<p><em>Hello</em></p>");

        Ok(())
    }

    #[tokio::test]
    async fn close_websockets_on_drop() -> Result<(), Box<dyn Error + Send + Sync>> {
        let server = new_server().await?;

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        drop(server);

        assert_matches!(websocket.next().await, Some(Ok(Message::Close(None))));

        assert_websocket_closed(&mut websocket).await;

        Ok(())
    }

    #[tokio::test]
    async fn queue_html_if_no_clients() -> Result<(), Box<dyn Error + Send + Sync>> {
        let server = new_server().await?;

        server.send("# Markdown").await?;

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        let message = timeout(Duration::from_secs(5), websocket.try_next())
            .await??
            .unwrap();
        assert!(message.is_text(), "message was not text: {:?}", message);
        assert_eq!(message.to_text().unwrap().trim(), "<h1>Markdown</h1>");

        Ok(())
    }

    #[tokio::test]
    async fn closed_websocket_removed_from_clients() -> Result<(), Box<dyn Error + Send + Sync>> {
        let server = new_server().await?;

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        websocket.close(None).await?;

        assert_websocket_closed(&mut websocket).await;

        server.send("# Markdown").await?;

        Ok(())
    }
}
