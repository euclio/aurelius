//! Functions for rendering markdown.

use hoedown::renderer::html;
use hoedown::{Markdown, Render};
use hoedown::{AUTOLINK, FENCED_CODE, TABLES, MATH, MATH_EXPLICIT};

/// Renders a markdown string to an HTML string.
///
/// This function enables the following extensions:
///
/// - Autolinking email addresses and URLs
/// - Fenced code blocks
/// - Tables
/// - Mathjax support
pub fn to_html(markdown: &str) -> String {
    let doc = Markdown::new(markdown).extensions(AUTOLINK | FENCED_CODE | TABLES | MATH | MATH_EXPLICIT);
    let mut html = html::Html::new(html::Flags::empty(), 0);
    html.render(&doc).to_str().unwrap().to_string()
}
