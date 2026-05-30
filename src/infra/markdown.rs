use pulldown_cmark::{Event, Options, Parser, html};

/// Render CommonMark to a sanitized HTML fragment safe to drop into a page.
/// Author input (announcements, etc.) goes through `ammonia` so any raw HTML
/// or `javascript:` URLs in the source can't escape into the rendered output.
pub fn render(source: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(source, options);
    let mut unsafe_html = String::with_capacity(source.len());
    html::push_html(&mut unsafe_html, parser);

    ammonia::clean(&unsafe_html)
}

/// Strip Markdown syntax and return just the prose. Used for previews and
/// excerpts so `**hello**` becomes `hello`, links flatten to their label
/// text, and code spans / fences keep their contents but lose the backticks.
/// Whitespace is collapsed so block boundaries don't leave double spaces.
pub fn to_plain_text(source: &str) -> String {
    let parser = Parser::new(source);
    let mut out = String::with_capacity(source.len());
    for event in parser {
        match event {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak | Event::End(_) => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
