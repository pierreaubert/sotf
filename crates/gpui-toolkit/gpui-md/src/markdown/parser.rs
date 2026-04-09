use comrak::{Arena, Options, parse_document};

/// Parse markdown text into a comrak AST with GFM extensions enabled.
///
/// Returns the arena and root node. The arena owns all AST nodes.
pub fn parse_markdown<'a>(arena: &'a Arena<comrak::nodes::AstNode<'a>>, text: &str) -> &'a comrak::nodes::AstNode<'a> {
    let mut options = Options::default();

    // Enable GFM extensions
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.header_ids = Some(String::new());

    // Enable source position tracking
    options.parse.default_info_string = None;

    parse_document(arena, text, &options)
}
