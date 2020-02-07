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
//! use aurelius::Server;
//!
//! let mut server = Server::bind("localhost:0")?;
//! println!("listening on {}", server.addr());
//!
//! server.open_browser()?;
//!
//! server.send(String::from("# Hello, world"));
//! # Ok::<_, Box<dyn std::error::Error>>(())
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

use std::error::Error;
use std::fs;
use std::io::{self, prelude::*};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::thread::JoinHandle;

use buf_redux::BufReader;
use crossbeam_channel::{select, Sender};
use crossbeam_utils::thread as crossbeam_thread;
use handlebars::Handlebars;
use httparse::{Request, Status, EMPTY_HEADER};
use include_dir::{include_dir, Dir};
use log::*;
use pulldown_cmark::{Options, Parser};
use serde::Serialize;
use sha1::{Digest, Sha1};
use tungstenite::{protocol::Role, Message, WebSocket};
use url::Url;

use crate::id_map::IdMap;

mod id_map;

const STATIC_FILES: Dir = include_dir!("static");

/// Markdown preview server.
///
/// Listens for HTTP connections and serves a page containing a live markdown preview. The page
/// contains JavaScript to open a websocket connection back to the server for rendering updates.
#[derive(Debug)]
pub struct Server {
    addr: SocketAddr,
    config: Arc<Mutex<Config>>,
    external_renderer: Option<Command>,
    md_clients: Arc<Mutex<IdMap<Sender<Signal>>>>,
    html: Arc<RwLock<Option<String>>>,
    /// Indicates whether the server should initiate shutdown.
    ///
    /// On drop, we want the server to clean up existing connections gracefully and stop listening
    /// for new connections. Unfortunately, closing a socket that is currently blocking in an
    /// `accept` loop is very platform-specific and potentially flaky. Instead, the `Drop` impl
    /// sets this flag to signal that the server should shut down on on the next connection, and
    /// then immediately opens a connection.
    shutdown: Arc<AtomicBool>,
    listener_join_handle: Option<JoinHandle<()>>,
}

impl Server {
    /// Binds the server to a specified address.
    ///
    /// Binding on port 0 will request a port assignment from the OS. Use `addr()` to query the
    /// assigned port.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let addr = listener.local_addr()?;

        info!("listening on {}", addr);

        let shutdown = Arc::new(AtomicBool::new(false));
        let md_clients = Arc::new(Mutex::new(IdMap::default()));
        let config = Arc::new(Mutex::new(Config::default()));
        let html = Arc::new(RwLock::new(None));

        let conn_shutdown = Arc::clone(&shutdown);
        let conn_md_clients = Arc::clone(&md_clients);
        let conn_config = Arc::clone(&config);
        let conn_html = Arc::clone(&html);

        let join_handle = thread::spawn(move || {
            crossbeam_thread::scope(|s| {
                for conn in listener.incoming() {
                    if conn_shutdown.load(Ordering::SeqCst) {
                        break;
                    }

                    let conn = match conn {
                        Ok(conn) => conn,
                        Err(_) => break,
                    };

                    let handler_config = Arc::clone(&conn_config);
                    let handler_md_clients = Arc::clone(&conn_md_clients);
                    let handler_html = Arc::clone(&conn_html);

                    s.spawn(|_| {
                        let handler = Handler {
                            conn,
                            config: handler_config,
                            md_clients: handler_md_clients,
                            html: handler_html,
                        };

                        if let Err(e) = handler.handle() {
                            match e.downcast_ref::<io::Error>() {
                                // MacOS may return EPROTOTYPE if a write occurs while the socket
                                // is being torn down. We could retry, but it's easier to just
                                // ignore it.
                                #[cfg(target_os = "macos")]
                                Some(e) if e.raw_os_error() == Some(41) => (),
                                Some(e)
                                    if e.kind() == io::ErrorKind::ConnectionReset
                                        || e.kind() == io::ErrorKind::BrokenPipe => (),
                                _ => panic!("unexpected error occurred: {}", e),
                            }
                        }
                    });
                }
            })
            .unwrap();
        });

        Ok(Server {
            addr,
            config,
            md_clients,
            html,
            external_renderer: None,
            shutdown,
            listener_join_handle: Some(join_handle),
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
    pub fn send(&mut self, markdown: String) -> io::Result<()> {
        let html = if let Some(renderer) = &mut self.external_renderer {
            let child = renderer.spawn()?;

            child.stdin.unwrap().write_all(markdown.as_bytes())?;

            let mut html = String::with_capacity(markdown.len());
            child.stdout.unwrap().read_to_string(&mut html)?;

            html
        } else {
            let mut html = String::with_capacity(markdown.len());
            let parser = Parser::new_ext(
                &markdown,
                Options::ENABLE_FOOTNOTES
                    | Options::ENABLE_TABLES
                    | Options::ENABLE_STRIKETHROUGH
                    | Options::ENABLE_TASKLISTS,
            );

            pulldown_cmark::html::push_html(&mut html, parser);

            html
        };

        *self.html.write().unwrap() = Some(html);

        for client in self.md_clients.lock().unwrap().values() {
            client.send(Signal::NewMarkdown).unwrap();
        }

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
        self.config.lock().unwrap().static_root = Some(root.into());
    }

    /// Set the highlight.js theme used for code blocks.
    ///
    /// Defaults to "github".
    pub fn set_highlight_theme(&mut self, theme: String) {
        self.config.lock().unwrap().highlight_theme = theme;
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
            match Url::parse(&stylesheet) {
                Ok(url) if url.scheme() == "http" || url.scheme() == "https" => links.push(url),
                _ => files.push(Path::new(stylesheet.trim_start_matches("file://"))),
            }
        }

        let mut config = self.config.lock().unwrap();

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
    /// use std::process::Command;
    /// use aurelius::Server;
    ///
    /// let mut server = Server::bind("localhost:0")?;
    ///
    /// let mut pandoc = Command::new("pandoc");
    /// pandoc.args(&["-f", "markdown", "-t", "html"]);
    ///
    /// server.set_external_renderer(pandoc);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    ///
    /// ```
    ///
    /// [`pulldown_cmark`]: https://github.com/raphlinus/pulldown-cmark
    /// [CommonMark]: https://commonmark.org/
    /// [`pandoc`]: https://pandoc.org/
    pub fn set_external_renderer(&mut self, mut command: Command) {
        command.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        self.external_renderer = Some(command);
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

impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr());

        // Shutdown all websocket connections.
        {
            let clients = std::mem::take(&mut *self.md_clients.lock().unwrap());

            for client in clients.values() {
                client.send(Signal::Close).unwrap();
            }
        }

        // Wait for connection threads to complete.
        self.listener_join_handle.take().unwrap().join().unwrap();
    }
}

enum Signal {
    NewMarkdown,
    Close,
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

#[derive(Debug)]
struct Handler {
    conn: TcpStream,
    config: Arc<Mutex<Config>>,
    md_clients: Arc<Mutex<IdMap<Sender<Signal>>>>,
    html: Arc<RwLock<Option<String>>>,
}

impl Handler {
    fn handle(mut self) -> Result<(), Box<dyn Error>> {
        let mut reader = BufReader::new(self.conn.try_clone()?);

        loop {
            let mut headers = [EMPTY_HEADER; 100];
            let mut req = Request::new(&mut headers);

            let buf = reader.fill_buf()?.to_owned();

            if buf.is_empty() {
                break;
            }

            let res = match req.parse(&buf) {
                Ok(res) => res,
                Err(_) => {
                    write!(self.conn, "HTTP/1.1 400 Bad Request\r\n")?;
                    return Ok(());
                }
            };

            match res {
                Status::Partial => {
                    reader.read_into_buf()?;
                    continue;
                }
                Status::Complete(n) => reader.consume(n),
            }

            if req
                .headers
                .iter()
                .any(|header| header.name == "Upgrade" && header.value == b"websocket")
            {
                self.serve_markdown_on_websocket(req)?;
                return Ok(());
            }

            self.serve_http(req)?;
            break;
        }

        Ok(())
    }

    fn serve_markdown_on_websocket(mut self, req: Request) -> Result<(), Box<dyn Error>> {
        let key = req.headers.iter().find_map(|header| {
            if header.name == "Sec-WebSocket-Key" {
                Some(header.value)
            } else {
                None
            }
        });

        let key = match key {
            Some(key) => key,
            None => {
                write!(self.conn, "HTTP/1.1 401 Bad Request\r\n")?;
                return Ok(());
            }
        };

        write!(self.conn, "HTTP/1.1 101 Switching Protocols\r\n")?;
        write!(self.conn, "Upgrade: websocket\r\n")?;
        write!(self.conn, "Connection: upgrade\r\n")?;
        write!(
            self.conn,
            "Sec-WebSocket-Accept: {}\r\n",
            websocket_accept(key)
        )?;
        write!(self.conn, "\r\n")?;
        self.conn.flush()?;

        let (md_tx, md_rx) = crossbeam_channel::unbounded();

        let client_id = self.md_clients.lock().unwrap().insert(md_tx);

        let mut writer = WebSocket::from_raw_socket(self.conn.try_clone()?, Role::Server, None);
        let mut reader = WebSocket::from_raw_socket(self.conn, Role::Server, None);

        // If there's HTML already present, send it to the client.
        {
            let html = self.html.read().unwrap();
            if let Some(html) = html.as_ref() {
                writer.write_message(Message::text(html))?;
            }
        }

        let clients = Arc::clone(&self.md_clients);
        thread::spawn(move || loop {
            match reader.read_message() {
                Err(_) => break,
                Ok(Message::Close(_)) => {
                    // The client may already be dropped by the time we get here.
                    clients.lock().unwrap().remove(client_id);
                    break;
                }
                Ok(_) => (),
            }
        });

        loop {
            select! {
                recv(md_rx) -> msg => {
                    // The server is being dropped.
                    if let Ok(Signal::Close) | Err(_) = msg {
                        // Ignore errors, since the socket may already be closed.
                        let _ = writer.close(None);
                        let _ = writer.write_pending();
                        break;
                    }

                    let html = self.html.read().unwrap();
                    writer.write_message(Message::text(html.as_ref().expect("no HTML present")))?;
                    writer.write_pending()?;
                }
            }
        }

        Ok(())
    }

    fn serve_http(&mut self, req: Request) -> io::Result<()> {
        let path = req.path.unwrap();

        if path.starts_with("/__/") {
            let path = path.trim_start_matches("/__/");

            match STATIC_FILES.get_file(path) {
                Some(file) => self.write_file_contents(file.path, file.contents)?,
                None => write!(self.conn, "HTTP/1.1 404 Not Found\r\n\r\n")?,
            }
        } else if path == "/" {
            #[derive(Debug, Serialize)]
            struct Data<'a> {
                remote_custom_css: &'a [Url],
                local_custom_css: &'a [String],
                highlight_theme: &'a str,
            }

            let html = {
                let config = self.config.lock().unwrap();
                let data = Data {
                    remote_custom_css: &config.css_links,
                    local_custom_css: &config.custom_styles,
                    highlight_theme: &config.highlight_theme,
                };
                Handlebars::new()
                    .render_template(include_str!("../templates/markdown_view.html"), &data)
                    .expect("invalid template syntax")
            };

            write!(self.conn, "HTTP/1.1 200 OK\r\n")?;
            write!(self.conn, "Connection: close\r\n")?;
            write!(self.conn, "Content-Type: text/html; charset=UTF-8\r\n")?;
            write!(self.conn, "\r\n")?;
            self.conn.write_all(html.as_bytes())?;
        } else {
            let root = self
                .config
                .lock()
                .unwrap()
                .deref()
                .static_root
                .clone()
                .map(|root| root.join(&url_path_to_file_path(path)));

            match root {
                Some(file_path) => self.write_file(&file_path)?,
                None => write!(self.conn, "HTTP/1.1 404 Not Found\r\n\r\n")?,
            }
        }

        self.conn.flush()?;

        Ok(())
    }

    fn write_file_contents(&mut self, path: impl AsRef<Path>, contents: &[u8]) -> io::Result<()> {
        write!(self.conn, "HTTP/1.1 200 OK\r\n")?;

        if let Some(mime_type) = mime_guess::from_path(path.as_ref()).first() {
            write!(self.conn, "Content-Type: {}\r\n", mime_type)?;
        }

        write!(self.conn, "Connection: close\r\n")?;
        write!(self.conn, "\r\n")?;
        self.conn.write_all(contents)?;

        Ok(())
    }

    fn write_file(&mut self, path: &Path) -> io::Result<()> {
        if let Ok(contents) = fs::read(&path) {
            self.write_file_contents(path, &contents)?;
        } else {
            write!(self.conn, "HTTP/1.1 404 Not Found\r\n\r\n")?;
        }

        Ok(())
    }
}

fn websocket_accept(key: &[u8]) -> String {
    static GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let mut accept = Sha1::new();
    accept.input(key);
    accept.input(GUID);

    base64::encode(&accept.result())
}

fn url_path_to_file_path(path: &str) -> PathBuf {
    path.trim_start_matches('/').split('/').collect()
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};

    use matches::assert_matches;
    use tungstenite::handshake::client::Request;
    use tungstenite::Message;
    use tungstenite::WebSocket;

    use super::Server;

    fn assert_websocket_closed<S: Read + Write>(websocket: &mut WebSocket<S>) {
        loop {
            match websocket.read_message() {
                Ok(Message::Close(_)) => (),
                Err(tungstenite::Error::ConnectionClosed) => break,
                other => panic!("unexpected connection state: {:?}", other),
            }
        }
    }

    #[test]
    fn uri_path_to_file_path() {
        assert_eq!(
            super::url_path_to_file_path("/file.txt"),
            Path::new("file.txt")
        );
        assert_eq!(
            super::url_path_to_file_path("/a/b/c/d"),
            vec!["a", "b", "c", "d"].iter().collect::<PathBuf>(),
        );
    }

    #[test]
    fn connect_http() -> Result<(), Box<dyn Error>> {
        let server = Server::bind("localhost:0")?;
        let addr = server.addr();

        reqwest::blocking::get(&format!("http://{}", addr))?;

        Ok(())
    }

    #[test]
    fn connect_websocket() -> Result<(), Box<dyn Error>> {
        let server = Server::bind("localhost:0")?;
        let addr = server.addr();

        let req = Request {
            url: format!("ws://{}", addr).parse()?,
            extra_headers: None,
        };

        tungstenite::connect(req).unwrap();

        Ok(())
    }

    #[test]
    fn send_with_no_clients() -> Result<(), Box<dyn Error>> {
        let mut server = Server::bind("localhost:0")?;

        server.send(String::from("This shouldn't hang")).unwrap();

        Ok(())
    }

    #[test]
    fn send_html() -> Result<(), Box<dyn Error>> {
        let mut server = Server::bind("localhost:0")?;
        let addr = server.addr();

        let req = Request {
            url: format!("ws://{}", addr).parse()?,
            extra_headers: None,
        };

        let (mut websocket, _) = tungstenite::connect(req)?;

        server.send(String::from("<p>Hello, world!</p>"))?;
        let message = websocket.read_message()?;
        assert_eq!(message.to_text()?, "<p>Hello, world!</p>");

        server.send(String::from("<p>Goodbye, world!</p>"))?;
        let message = websocket.read_message()?;
        assert_eq!(message.to_text()?, "<p>Goodbye, world!</p>");

        Ok(())
    }

    #[test]
    fn send_markdown() -> Result<(), Box<dyn Error>> {
        let mut server = Server::bind("localhost:0")?;
        let addr = server.addr();

        let req = Request {
            url: format!("ws://{}", addr).parse()?,
            extra_headers: None,
        };

        let (mut websocket, _) = tungstenite::connect(req)?;

        server.send(String::from("*Hello*"))?;
        let message = websocket.read_message()?;
        assert_eq!(message.to_text()?.trim(), "<p><em>Hello</em></p>");

        Ok(())
    }

    #[test]
    fn close_websockets_on_drop() -> Result<(), Box<dyn Error>> {
        let server = Server::bind("localhost:0")?;
        let addr = server.addr();

        let req = Request {
            url: format!("ws://{}", addr).parse()?,
            extra_headers: None,
        };

        let (mut websocket, _) = tungstenite::connect(req).unwrap();

        drop(server);

        assert_websocket_closed(&mut websocket);

        Ok(())
    }

    #[test]
    fn queue_html_if_no_clients() -> Result<(), Box<dyn Error>> {
        let mut server = Server::bind("localhost:0")?;
        let addr = server.addr();

        server.send(String::from("# Markdown"))?;

        let req = Request {
            url: format!("ws://{}", addr).parse()?,
            extra_headers: None,
        };

        let (mut websocket, _) = tungstenite::connect(req).unwrap();

        let message = websocket.read_message().unwrap();
        assert!(message.is_text(), "message was not text: {:?}", message);
        assert_eq!(message.to_text().unwrap().trim(), "<h1>Markdown</h1>");
        websocket.close(None).unwrap();

        assert_websocket_closed(&mut websocket);

        Ok(())
    }

    #[test]
    fn closed_websocket_removed_from_clients() -> Result<(), Box<dyn Error>> {
        let mut server = Server::bind("localhost:0")?;
        let addr = server.addr();

        let req = Request {
            url: format!("ws://{}", addr).parse()?,
            extra_headers: None,
        };

        let (mut websocket, _) = tungstenite::connect(req)?;

        websocket.close(None)?;
        websocket.write_pending().unwrap();

        assert_websocket_closed(&mut websocket);

        server.send(String::from("# Markdown")).unwrap();

        assert_matches!(
            websocket.read_message(),
            Err(tungstenite::Error::AlreadyClosed)
        );

        Ok(())
    }
}
