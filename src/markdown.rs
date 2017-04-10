//! Functions for rendering markdown.

use pulldown_cmark::{Parser, html};
use pulldown_cmark::{OPTION_ENABLE_TABLES, OPTION_ENABLE_FOOTNOTES};

/// Renders a markdown string to an HTML string.
///
/// This function renders markdown according to the [CommonMark] spec with the following extensions
/// enabled:
///
/// - Tables
/// - Footnotes
///
/// [CommonMark]: http://commonmark.org/
pub fn to_html(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, OPTION_ENABLE_TABLES | OPTION_ENABLE_FOOTNOTES);

    let mut output = String::new();
    html::push_html(&mut output, parser);

    output
}
