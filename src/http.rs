//! Contains the HTTP server component.

use std::fs;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::{Arc, Mutex};

use handlebars_iron::{Template, HandlebarsEngine, DirectorySource};
use iron::prelude::*;
use iron::{self, Handler, status};
use mount::Mount;
use serde_json::Value;
use staticfile::Static;

use Config;
use markdown;

lazy_static! {
    static ref CRATE_ROOT: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
}

/// The HTTP server.
///
/// The server listens on the provided port, rendering the markdown preview when a GET request is
/// received at the server root.
pub struct Server<'a> {
    config: &'a Config,
}

impl<'a> Server<'a> {
    /// Creates a new server that listens on socket address `addr`.
    pub fn new(config: &'a Config) -> Server<'a> {
        Server {
            config: config,
        }
    }

    /// Starts the server.
    ///
    /// Once a connection is received, the client will initiate WebSocket connections on
    /// `websocket_port`. If `initial_markdown` is present, it will be displayed on the first
    /// connection.
    pub fn listen<A>(self, address: A, websocket_port: u16) -> io::Result<Listening>
            where A: ToSocketAddrs {
        let working_directory = Arc::new(Mutex::new(self.config.working_directory.clone()));

        let handler = create_handler(MarkdownPreview {
            template_data: json!({
                "websocket_port": websocket_port,
                "initial_markdown": markdown::to_html(&self.config.initial_markdown),
                "highlight_theme": self.config.highlight_theme,
                "custom_css": self.config.custom_css,
            }),
            working_directory: working_directory.clone(),
        });

        let listening = Listening {
            working_directory: working_directory,
            listening: Iron::new(handler)
                .http(address)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        };

        Ok(listening)
    }
}

pub struct Listening {
    listening: iron::Listening,
    working_directory: Arc<Mutex<PathBuf>>,
}

impl Listening {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.listening.socket)
    }

    pub fn change_working_directory<P>(&mut self, dir: P) where P: AsRef<Path> {
        let mut working_directory = self.working_directory.lock().unwrap();
        *working_directory = dir.as_ref().to_owned();
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        self.listening.close().unwrap();
    }
}

/// Wraps the markdown handler with other middleware, such as template rendering and static file
/// serving.
fn create_handler(previewer: MarkdownPreview) -> Box<Handler> {
    let mut chain = Chain::new(previewer);

    let mut hbse = HandlebarsEngine::new();
    hbse.add(Box::new(DirectorySource::new(CRATE_ROOT.join("templates/").to_str().unwrap(),
                                           ".html")));
    if let Err(r) = hbse.reload() {
        panic!("{}", r);
    }

    chain.link_after(hbse);

    let mut mount = Mount::new();
    mount.mount("/", chain);
    mount.mount("/_static", Static::new(CRATE_ROOT.join("static")));

    Box::new(mount)
}

struct MarkdownPreview {
    template_data: Value,
    working_directory: Arc<Mutex<PathBuf>>,
}

impl Handler for MarkdownPreview {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let url_path = req.url.path().join(&MAIN_SEPARATOR.to_string());

        if url_path.is_empty() {
            Ok(Response::with((Template::new("markdown_view", &self.template_data), status::Ok)))
        } else {
            let local_cwd = self.working_directory.clone();
            let path = local_cwd.lock().unwrap().join(&url_path);

            match fs::metadata(&path) {
                Ok(ref attr) if attr.is_file() => return Ok(Response::with((path, status::Ok))),
                Err(ref e) if e.kind() != io::ErrorKind::NotFound => {
                    debug!("Error getting metadata for file: '{:?}': {:?}", path, e);
                },
                _ => (),
            }

            Ok(Response::with(status::NotFound))
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate iron_test;

    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use iron::Headers;
    use self::iron_test::request;

    use Config;
    use markdown;
    use super::MarkdownPreview;

    #[test]
    fn simple() {
        let config = Config::default();
        let handler = super::create_handler(MarkdownPreview {
            template_data: json!({
                "websocket_port": 1337,
                "initial_markdown": markdown::to_html(&config.initial_markdown),
                "highlight_theme": config.highlight_theme,
                "custom_css": config.custom_css,
            }),
            working_directory: Arc::new(Mutex::new(PathBuf::new())),
        });

        let response = request::get("http://localhost:3000/", Headers::new(), &handler).unwrap();
        assert!(response.status.unwrap().is_success(), "could not load index");

        let response = request::get("http://localhost:3000/_static/js/markdown_client.js",
                                    Headers::new(),
                                    &handler)
                .unwrap();
        assert!(response.status.unwrap().is_success(), "static file not found");

        let response = request::get("http://localhost:3000/non-existent",
                                    Headers::new(),
                                    &handler)
                .unwrap();
        assert!(response.status.unwrap().is_client_error(), "found non-existent file");
    }
}
