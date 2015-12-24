//! Contains the HTTP server component.

use std::collections::HashMap;
use std::env;
use std::path::Path;

use nickel::{Nickel,StaticFilesHandler};

use markdown;

/// The HTTP server.
///
/// The server listens on the provided port, rendering the markdown preview when a GET request is
/// received at the server root.
pub struct Server {

    /// The port that the server is listening on.
    pub port: u16,
}

impl Server {
    /// Creates a new server that listens on port `port`.
    pub fn new(port: u16) -> Server {
        Server { port: port }
    }

    /// Starts the server.
    ///
    /// Once a connection is received, the client will initiate WebSocket connections on
    /// `websocket_port`. If `initial_markdown` is present, it will be displayed on the first
    /// connection.
    ///
    /// This method does not return.
    pub fn start(&self, websocket_port: u16, initial_markdown: String, highlight_theme: String) {
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

        let mut static_dir = root.to_path_buf();
        static_dir.push("static");
        assert!(static_dir.is_absolute());
        server.utilize(StaticFilesHandler::new(static_dir.to_str().unwrap()));

        server.listen(("localhost", self.port));
    }
}
