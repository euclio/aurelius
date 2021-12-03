//! [aurelius](https://github.com/euclio/aurelius) is a complete solution for live-previewing
//! markdown as HTML.
//!
//! This crate provides a server that can render and update an HTML preview of markdown without a
//! client-side refresh. Upon receiving an HTTP request, the server responds with an HTML page
//! containing a rendering of supplied markdown. Client-side JavaScript then initiates a WebSocket
//! connection which allows the server to push changes to the client.
//!
//! This crate was designed to power [vim-markdown-composer], a markdown preview plugin for
//! [Neovim](http://neovim.io), but it may be used to implement similar plugins for any editor.
//! See [vim-markdown-composer] for a real-world usage example.
//!
//! # Example
//!
//! ```no_run
//! use std::net::SocketAddr;
//! use aurelius::Server;
//!
//! # tokio_test::block_on(async {
//!     let addr = "127.0.0.1:1337".parse::<SocketAddr>()?;
//!     let mut server = Server::bind(&addr).await?;
//!
//!     server.open_browser()?;
//!
//!     server.send("# Hello, world!");
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
use std::convert::Infallible;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, RwLock};

use hyper::service::make_service_fn;
use log::*;
use pulldown_cmark::{Options, Parser};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio::sync::watch::{self, Sender};
use url::Url;

mod service;

use service::WebsocketBroadcastService;

/// Markdown preview server.
///
/// Listens for HTTP connections and serves a page containing a live markdown preview. The page
/// contains JavaScript to open a websocket connection back to the server for rendering updates.
#[derive(Debug)]
pub struct Server {
    addr: SocketAddr,
    config: Arc<RwLock<Config>>,
    external_renderer: Option<RefCell<Command>>,
    tx: Sender<String>,
    _shutdown_tx: oneshot::Sender<()>,
}

impl Server {
    /// Binds the server to a specified address.
    ///
    /// Binding to port 0 will request a port assignment from the OS. Use `addr()` to query the
    /// assigned port.
    pub async fn bind(addr: &SocketAddr) -> anyhow::Result<Self> {
        let (tx, rx) = watch::channel(String::new());
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let config = Arc::default();

        let make_service_config = Arc::clone(&config);
        let make_service = make_service_fn(move |_conn| {
            let html_rx = rx.clone();
            let service_config = Arc::clone(&make_service_config);

            async move {
                Ok::<_, Infallible>(WebsocketBroadcastService {
                    html_rx,
                    config: Arc::clone(&service_config),
                })
            }
        });

        let http_server = hyper::Server::try_bind(addr)?.serve(make_service);

        let addr = http_server.local_addr();
        info!("listening on {:?}", addr);

        let http_server = http_server.with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });

        tokio::spawn(http_server);

        Ok(Server {
            addr,
            config,
            external_renderer: None,
            tx,
            _shutdown_tx: shutdown_tx,
        })
    }

    /// Returns the socket address that the server is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Publish new markdown to be rendered by the server.
    ///
    /// The new HTML will be sent to all connected websocket clients.
    ///
    /// # Errors
    ///
    /// This method forwards errors from an external renderer, if set. Otherwise, the method is
    /// infallible.
    pub async fn send(&self, markdown: &str) -> io::Result<()> {
        let html = if let Some(renderer) = &self.external_renderer {
            let child = renderer.borrow_mut().spawn()?;

            child.stdin.unwrap().write_all(markdown.as_bytes()).await?;

            let mut html = String::with_capacity(markdown.len());
            child.stdout.unwrap().read_to_string(&mut html).await?;

            html
        } else {
            let mut html = String::with_capacity(markdown.len());
            let parser = Parser::new_ext(
                markdown,
                Options::ENABLE_FOOTNOTES
                    | Options::ENABLE_TABLES
                    | Options::ENABLE_STRIKETHROUGH
                    | Options::ENABLE_TASKLISTS,
            );

            pulldown_cmark::html::push_html(&mut html, parser);

            html
        };

        self.tx.send_replace(html);

        Ok(())
    }

    /// Set the directory that static files will be served from.
    ///
    /// This can be thought of as the "working directory" of the server. Any HTTP requests with
    /// non-root paths will be joined to this folder and used to serve files from the filesystem.
    /// Typically this is used to serve image links relative to the markdown file.
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
            match Url::parse(stylesheet) {
                Ok(url) if url.scheme() == "http" || url.scheme() == "https" => links.push(url),
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

    /// Set an external program to use for rendering the markdown.
    ///
    /// By default, aurelius uses [`pulldown_cmark`] to render markdown in-process.
    /// `pulldown-cmark` is an extremely fast, [CommonMark]-compliant parser that is sufficient
    /// for most use-cases. However, other markdown renderers may provide additional features.
    ///
    /// The `Command` supplied to this function should expect markdown on stdin and print HTML on
    /// stdout.
    ///
    /// # Example
    ///
    /// To use [`pandoc`] to render markdown:
    ///
    ///
    /// ```no_run
    /// # async fn dox() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::net::SocketAddr;
    /// use tokio::process::Command;
    /// use aurelius::Server;
    ///
    /// let addr = "127.0.0.1:1337".parse::<SocketAddr>()?;
    /// let mut server = Server::bind(&addr).await?;
    ///
    /// let mut pandoc = Command::new("pandoc");
    /// pandoc.args(&["-f", "markdown", "-t", "html"]);
    ///
    /// server.set_external_renderer(pandoc);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`pulldown_cmark`]: https://github.com/raphlinus/pulldown-cmark
    /// [CommonMark]: https://commonmark.org/
    /// [`pandoc`]: https://pandoc.org/
    pub fn set_external_renderer(&mut self, mut command: Command) {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        self.external_renderer = Some(RefCell::new(command));
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

#[derive(Debug)]
struct Config {
    static_root: Option<PathBuf>,
    highlight_theme: String,
    css_links: Vec<Url>,
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

    use super::Server;

    async fn new_server() -> anyhow::Result<Server> {
        let addr = lookup_host("localhost:0").await?.next().unwrap();
        Ok(Server::bind(&addr).await?)
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

        let body = reqwest::get(&format!("http://{}", server.addr()))
            .await?
            .text()
            .await?;

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
    async fn send_with_no_clients() -> anyhow::Result<()> {
        let server = new_server().await?;

        server.send("This shouldn't hang").await?;

        Ok(())
    }

    #[tokio::test]
    async fn send_html() -> anyhow::Result<()> {
        let server = new_server().await?;

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        server.send("<p>Hello, world!</p>").await?;
        let message = websocket.next().await.unwrap()?;
        assert_eq!(message.to_text()?, "<p>Hello, world!</p>");

        server.send("<p>Goodbye, world!</p>").await?;
        let message = websocket.next().await.unwrap()?;
        assert_eq!(message.to_text()?, "<p>Goodbye, world!</p>");

        Ok(())
    }

    #[tokio::test]
    async fn send_markdown() -> anyhow::Result<()> {
        let server = new_server().await?;

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        server.send("*Hello*").await?;
        let message = websocket.next().await.unwrap()?;
        assert_eq!(message.to_text()?.trim(), "<p><em>Hello</em></p>");

        Ok(())
    }

    #[tokio::test]
    async fn close_websockets_on_drop() -> Result<(), Box<dyn Error>> {
        let server = new_server().await?;

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        drop(server);

        assert_matches!(websocket.next().await, Some(Ok(Message::Close(None))));

        assert_websocket_closed(&mut websocket).await;

        Ok(())
    }

    #[tokio::test]
    async fn queue_html_if_no_clients() -> Result<(), Box<dyn Error>> {
        let _ = env_logger::builder().is_test(true).try_init();

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
    async fn closed_websocket_removed_from_clients() -> Result<(), Box<dyn Error>> {
        let server = new_server().await?;

        let (mut websocket, _) =
            async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

        websocket.close(None).await?;

        assert_websocket_closed(&mut websocket).await;

        server.send("# Markdown").await?;

        Ok(())
    }
}
