//! Contains the HTTP server component.

use std::fs::{self, File};
use std::io::prelude::*;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::{Arc, Mutex};

use handlebars_iron::{DirectorySource, HandlebarsEngine, Template};
use iron::prelude::*;
use iron::{self, status, Handler};
use mount::Mount;
use staticfile::Static;

lazy_static! {
    static ref CRATE_ROOT: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
}

/// The HTTP server.
///
/// The server listens on the provided port, rendering the markdown preview when a GET request is
/// received at the server root.
pub struct Server {
    websocket_port: u16,
    styles: StyleConfig,
    working_directory: PathBuf,
}

pub struct StyleConfig {
    pub highlight_theme: String,
    pub css: Vec<String>,
}

/// Data that is passed to the HTML template when rendering.
#[derive(Debug, Serialize)]
struct TemplateData {
    websocket_port: u16,
    initial_html: String,
    highlight_theme: String,

    /// URLs pointing to remote CSS resources.
    remote_custom_css: Vec<String>,

    /// Full text of local CSS resources.
    local_custom_css: Vec<String>,
}

impl Server {
    /// Creates a new server that listens on socket address `addr`.
    pub fn new(working_directory: PathBuf, websocket_port: u16, styles: StyleConfig) -> Server {
        Server {
            working_directory: working_directory,
            websocket_port: websocket_port,
            styles: styles,
        }
    }

    /// Starts the server.
    ///
    /// Once a connection is received, the client will initiate WebSocket connections on
    /// `websocket_port`. If `initial_markdown` is present, it will be displayed on the first
    /// connection.
    pub fn listen<A>(self, address: A, initial_html: &str) -> io::Result<Listening>
    where
        A: ToSocketAddrs,
    {
        let working_directory = Arc::new(Mutex::new(self.working_directory));

        // This is a bit of a hack, should probably use real URLs here.
        let (remote_custom_css, local_custom_css): (Vec<String>, Vec<String>) = self.styles
            .css
            .into_iter()
            .partition(|css| !css.starts_with("file://"));

        let local_custom_css = local_custom_css
            .into_iter()
            .flat_map(|file_uri| File::open(file_uri.trim_start_matches("file://")).ok())
            .map(|mut file| {
                let mut css = String::new();
                file.read_to_string(&mut css).unwrap();
                css
            })
            .collect();

        let handler = create_handler(MarkdownPreview {
            template_data: TemplateData {
                websocket_port: self.websocket_port,
                initial_html: initial_html.to_owned(),
                highlight_theme: self.styles.highlight_theme,
                remote_custom_css,
                local_custom_css,
            },
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

#[derive(Debug)]
pub struct Listening {
    listening: iron::Listening,
    working_directory: Arc<Mutex<PathBuf>>,
}

impl Listening {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.listening.socket)
    }

    pub fn change_working_directory<P>(&mut self, dir: P)
    where
        P: AsRef<Path>,
    {
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
    hbse.add(Box::new(DirectorySource::new(
        CRATE_ROOT.join("templates/").to_str().unwrap(),
        ".html",
    )));
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
    template_data: TemplateData,
    working_directory: Arc<Mutex<PathBuf>>,
}

impl Handler for MarkdownPreview {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let url_path = req.url.path().join(&MAIN_SEPARATOR.to_string());

        if url_path.is_empty() {
            Ok(Response::with((
                Template::new("markdown_view", &self.template_data),
                status::Ok,
            )))
        } else {
            let local_cwd = self.working_directory.clone();
            let path = local_cwd.lock().unwrap().join(&url_path);

            match fs::metadata(&path) {
                Ok(ref attr) if attr.is_file() => return Ok(Response::with((path, status::Ok))),
                Err(ref e) if e.kind() != io::ErrorKind::NotFound => {
                    debug!("Error getting metadata for file: '{:?}': {:?}", path, e);
                }
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

    use config;
    use super::{MarkdownPreview, TemplateData};

    #[test]
    fn simple() {
        let handler = super::create_handler(MarkdownPreview {
            template_data: TemplateData {
                websocket_port: 1337,
                initial_html: String::new(),
                highlight_theme: config::DEFAULT_HIGHLIGHT_THEME.to_string(),
                remote_custom_css: vec![config::DEFAULT_CSS.to_string()],
                local_custom_css: vec![],
            },
            working_directory: Arc::new(Mutex::new(PathBuf::new())),
        });

        let response = request::get("http://localhost:3000/", Headers::new(), &handler).unwrap();
        assert!(
            response.status.unwrap().is_success(),
            "could not load index"
        );

        let response = request::get(
            "http://localhost:3000/_static/js/markdown_client.js",
            Headers::new(),
            &handler,
        ).unwrap();
        assert!(
            response.status.unwrap().is_success(),
            "static file not found"
        );

        let response = request::get(
            "http://localhost:3000/non-existent",
            Headers::new(),
            &handler,
        ).unwrap();
        assert!(
            response.status.unwrap().is_client_error(),
            "found non-existent file"
        );
    }

    #[test]
    fn vendored() {
        let handler = super::create_handler(MarkdownPreview {
            template_data: TemplateData {
                websocket_port: 1337,
                initial_html: String::new(),
                highlight_theme: config::DEFAULT_HIGHLIGHT_THEME.to_string(),
                remote_custom_css: vec![config::DEFAULT_CSS.to_string()],
                local_custom_css: vec![],
            },
            working_directory: Arc::new(Mutex::new(PathBuf::new())),
        });

        let response = request::get(
            "http://localhost:3000/_static/vendor/highlight.js/highlight.pack.js",
            Headers::new(),
            &handler,
        ).unwrap();
        assert!(
            response.status.unwrap().is_success(),
            "vendored file not found"
        );
    }
}
