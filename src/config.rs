use std::env;
use std::path::PathBuf;

pub const DEFAULT_HIGHLIGHT_THEME: &str = "github";
pub const DEFAULT_CSS: &str = "/_static/vendor/github-markdown-css/github-markdown.css";

/// `std::process::Command` doesn't implement `Clone`, so we have to make a lightweight wrapper
/// type that we can pass to that constructor directly.
pub type ExternalRenderer = (String, Vec<String>);

/// Configuration for the server.
#[derive(Debug, Clone)]
pub struct Config {
    /// The initial markdown that should be displayed when the server has not received markdown
    /// yet.
    pub initial_markdown: Option<String>,

    /// The initial working directory for the server. Defaults to the current process' working
    /// directory.
    ///
    /// Static files will be served relative to this directory.
    pub working_directory: PathBuf,

    /// The highlight.js theme that should be used for syntax highlighting. Defaults to the
    /// "github" theme.
    pub highlight_theme: String,

    /// A process that should be used to render markdown from stdin on stdout.
    pub external_renderer: Option<ExternalRenderer>,

    /// Custom CSS styles that should be included in the HTML output. Defaults to GitHub's markdown
    /// CSS styles.
    pub custom_css: Vec<String>,

    /// The port that the HTTP server should listen on.
    pub http_port: u16,

    /// The port that the websocket server should listen on.
    pub websocket_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            working_directory: env::current_dir()
                .expect("no current working directory")
                .to_owned(),
            initial_markdown: None,
            highlight_theme: String::from(DEFAULT_HIGHLIGHT_THEME),
            external_renderer: None,
            custom_css: vec![String::from(DEFAULT_CSS)],
            http_port: 0,
            websocket_port: 0,
        }
    }
}
