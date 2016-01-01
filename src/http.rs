//! Contains the HTTP server component.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use porthole;
use nickel::{Nickel, StaticFilesHandler};

use markdown;

/// The HTTP server.
///
/// The server listens on the provided port, rendering the markdown preview when a GET request is
/// received at the server root.
pub struct Server {
    local_addr: SocketAddr,

    /// The "current working directory" of the server. Any static file requests will be joined to
    /// this directory.
    cwd: Arc<Mutex<PathBuf>>,
}

impl Server {
    /// Creates a new server that listens on socket address `addr`.
    pub fn new<A, P>(addr: A, working_directory: P) -> Server
        where A: ToSocketAddrs,
              P: AsRef<Path>
    {
        let socket_addr = addr.to_socket_addrs()
                              .unwrap()
                              .map(|addr| {
                                  if addr.port() == 0 {
                                      let unused_port = porthole::open().unwrap();
                                      format!("localhost:{}", unused_port)
                                          .to_socket_addrs()
                                          .unwrap()
                                          .next()
                                          .unwrap()
                                  } else {
                                      addr
                                  }
                              })
                              .next()
                              .unwrap();

        Server {
            local_addr: socket_addr,
            cwd: Arc::new(Mutex::new(working_directory.as_ref().to_owned())),
        }
    }

    pub fn change_working_directory<P>(&mut self, dir: P)
        where P: AsRef<Path>
    {
        let mut cwd = self.cwd.lock().unwrap();
        *cwd = dir.as_ref().to_owned();
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local_addr)
    }

    fn listen_forever(local_addr: SocketAddr,
                      websocket_port: u16,
                      initial_markdown: String,
                      highlight_theme: String,
                      current_working_directory: Arc<Mutex<PathBuf>>) {
        let mut server = Nickel::new();

        let mut data = HashMap::new();
        data.insert("websocket_port", websocket_port.to_string());

        data.insert("initial_markdown", markdown::to_html(&initial_markdown));
        data.insert("highlight_theme", highlight_theme);

        // We need to figure out the crate root, so we can pass absolute paths into the nickel
        // APIs.
        let root = {
            let crate_root = Path::new(file!()).parent().unwrap().parent().unwrap();
            if crate_root.is_absolute() {
                crate_root.to_owned()
            } else {
                let mut current_dir = env::current_dir().unwrap();
                current_dir.push(crate_root);
                current_dir.to_owned()
            }
        };

        let mut markdown_view = root.to_path_buf();
        markdown_view.push("templates/markdown_view.html");

        server.utilize(router! {
            get "/" => |_, response| {
                return response.render(markdown_view.to_str().unwrap(), &data);
            }
        });

        let local_cwd = current_working_directory.clone();
        server.utilize(middleware! { |request, response|
            let path = request.path_without_query().map(|path| {
                path[1..].to_owned()
            });

            if let Some(path) = path {
                let path = local_cwd.lock().unwrap().clone().join(path);
                match fs::metadata(&path) {
                    Ok(ref attr) if attr.is_file() => return response.send_file(&path),
                    Err(ref e) if e.kind() != io::ErrorKind::NotFound => {
                        debug!("Error getting metadata for file '{:?}': {:?}",
                                                                  path, e)
                    }
                    _ => {}
                }
            };
        });

        let mut static_dir = root.to_path_buf();
        static_dir.push("static");
        assert!(static_dir.is_absolute());
        server.utilize(StaticFilesHandler::new(static_dir.to_str().unwrap()));

        server.listen(local_addr);
    }

    /// Starts the server.
    ///
    /// Once a connection is received, the client will initiate WebSocket connections on
    /// `websocket_port`. If `initial_markdown` is present, it will be displayed on the first
    /// connection.
    pub fn start(&self, websocket_port: u16, initial_markdown: String, highlight_theme: String) {
        let current_working_directory = self.cwd.clone();
        let local_addr = self.local_addr;
        thread::spawn(move || {
            Self::listen_forever(local_addr,
                                 websocket_port,
                                 initial_markdown,
                                 highlight_theme,
                                 current_working_directory);
        });
    }
}
