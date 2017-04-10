//! Functions for rendering markdown.

use pulldown_cmark::{Parser, html};
use pulldown_cmark::{OPTION_ENABLE_TABLES, OPTION_ENABLE_FOOTNOTES};

/// Renders a markdown string to an HTML string.
///
/// This function enables the following extensions:
///
/// - Autolinking email addresses and URLs
/// - Fenced code blocks
/// - Tables
pub fn to_html(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, OPTION_ENABLE_TABLES | OPTION_ENABLE_FOOTNOTES);

    let mut output = String::new();
    html::push_html(&mut output, parser);

    output
}
