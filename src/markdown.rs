//! Functions for rendering markdown.

use std::io::prelude::*;
use std::process::{Command, Stdio};

use pulldown_cmark::{html, Parser};
use pulldown_cmark::{OPTION_ENABLE_FOOTNOTES, OPTION_ENABLE_TABLES};

use errors::*;

/// Renders a markdown string to an HTML string using an in-process CommonMark renderer.
///
/// # Notes
///
/// This function renders markdown according to the [CommonMark] spec with the following extensions
/// enabled:
///
/// - Tables
/// - Footnotes
///
/// [CommonMark]: http://commonmark.org/
pub fn to_html_cmark(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, OPTION_ENABLE_TABLES | OPTION_ENABLE_FOOTNOTES);

    let mut output = String::new();
    html::push_html(&mut output, parser);

    output
}

/// Renders a markdown string to an HTML string using an external process.
///
/// The process should listen for markdown on stdin and output HTML on stdout.
///
/// # Notes
///
/// The output must be encoded as UTF-8.
///
/// # Examples
///
/// ```no_run
/// use std::process::Command;
///
/// use aurelius::markdown;
///
/// let mut pandoc = Command::new("pandoc");
/// pandoc.args(&["-f", "markdown"]).args(&["-t", "html"]);
///
/// let html = markdown::to_html_external(pandoc, "# Hello, world!")
///     .expect("error executing process");
/// println!("{}", html);
/// ```
pub fn to_html_external(mut command: Command, markdown: &str) -> Result<String> {
    let child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    child.stdin.unwrap().write_all(markdown.as_bytes())?;

    let mut html = String::new();
    child.stdout.unwrap().read_to_string(&mut html)?;

    Ok(html)
}
