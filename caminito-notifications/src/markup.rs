//! A body's markup, made safe for Pango. The spec allows `<b>`, `<i>`, `<u>`, `<a href>` and
//! `<img>`, and apps send plain text with a bare `&` or `<` too. What Pango can show of the
//! allowed tags is kept, with `<a>` as GtkLabel's link, and everything else is shown as the
//! text it is: a tag Pango doesn't know, or one left open, would make it refuse the whole body.

/// The tags kept as they are.
const STYLES: [&str; 3] = ["b", "i", "u"];

/// `body` as Pango markup that shows what the spec's markup means.
pub(crate) fn sanitize(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    // The tags open at this point, innermost last.
    let mut open: Vec<&'static str> = Vec::new();
    let mut rest = body;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some((tag, after)) = tag(rest) {
                match tag {
                    Tag::Open(name, attributes) => {
                        out.push('<');
                        out.push_str(name);
                        out.push_str(&attributes);
                        out.push('>');
                        open.push(name);
                    }
                    Tag::Close(name) => {
                        // A close for a tag that isn't innermost closes those inside it too,
                        // and one for a tag that isn't open is dropped.
                        if let Some(position) = open.iter().rposition(|open| *open == name) {
                            for inner in open.drain(position..).rev() {
                                close(&mut out, inner);
                            }
                        }
                    }
                    Tag::Dropped => {}
                }
                rest = after;
                continue;
            }
            out.push_str("&lt;");
        } else if c == '&' {
            if let Some(length) = entity(rest) {
                out.push_str(&rest[..length]);
                rest = &rest[length..];
                continue;
            }
            out.push_str("&amp;");
        } else if c == '>' {
            out.push_str("&gt;");
        } else {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    for inner in open.into_iter().rev() {
        close(&mut out, inner);
    }
    out
}

fn close(out: &mut String, tag: &str) {
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

enum Tag {
    /// A tag Pango has, with its attributes as Pango is to read them.
    Open(&'static str, String),
    Close(&'static str),
    /// A tag the spec allows that Pango can't show, as `<img>`, or a `<br>`.
    Dropped,
}

/// The tag `text` starts with, if it's one the spec allows, and the text after it.
fn tag(text: &str) -> Option<(Tag, &str)> {
    let end = text.find('>')?;
    let inside = &text[1..end];
    let after = &text[end + 1..];
    let (closing, inside) = match inside.strip_prefix('/') {
        Some(inside) => (true, inside),
        None => (false, inside),
    };
    let inside = inside.trim_end_matches('/').trim();
    let (name, attributes) = inside
        .split_once(char::is_whitespace)
        .unwrap_or((inside, ""));
    let name = name.to_ascii_lowercase();
    if let Some(style) = STYLES.iter().find(|style| **style == name) {
        if !attributes.trim().is_empty() {
            return None;
        }
        let tag = if closing {
            Tag::Close(style)
        } else {
            Tag::Open(style, String::new())
        };
        return Some((tag, after));
    }
    match name.as_str() {
        "a" if closing => Some((Tag::Close("a"), after)),
        "a" => {
            let href = attribute(attributes, "href")?;
            Some((
                Tag::Open("a", format!(" href=\"{}\"", escape(&href))),
                after,
            ))
        }
        "img" | "br" => Some((Tag::Dropped, after)),
        _ => None,
    }
}

/// The value of the attribute `name` in `attributes`, quoted with `"` or `'`, with its
/// entities as they're written.
fn attribute(attributes: &str, name: &str) -> Option<String> {
    let mut rest = attributes.trim_start();
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let after = after.trim_start();
        let quote = after.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let value_end = after[1..].find(quote)? + 1;
        if key.trim().eq_ignore_ascii_case(name) {
            return Some(after[1..value_end].to_owned());
        }
        rest = after[value_end + 1..].trim_start();
    }
    None
}

/// `text` for an attribute's value: its entities kept, and what would end the value escaped.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(length) = entity(rest) {
            out.push_str(&rest[..length]);
            rest = &rest[length..];
            continue;
        }
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The length of the XML entity `text` starts with, if it starts with one Pango reads.
fn entity(text: &str) -> Option<usize> {
    let end = text.find(';')?;
    let name = &text[1..end];
    let known = match name.strip_prefix('#') {
        Some(number) => match number.strip_prefix(['x', 'X']) {
            Some(hex) => !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()),
            None => !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()),
        },
        None => matches!(name, "amp" | "lt" | "gt" | "quot" | "apos"),
    };
    known.then_some(end + 1)
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    #[test]
    fn the_specs_styles_and_links_are_kept() {
        assert_eq!(
            sanitize("<b>bold</b> <i>it</i> <u>under</u>"),
            "<b>bold</b> <i>it</i> <u>under</u>"
        );
        assert_eq!(
            sanitize("see <a href=\"https://example.com/?a=1&amp;b=2\">this</a>"),
            "see <a href=\"https://example.com/?a=1&amp;b=2\">this</a>"
        );
        assert_eq!(
            sanitize("<a href='https://x.org/?q=a&b'>x</a>"),
            "<a href=\"https://x.org/?q=a&amp;b\">x</a>"
        );
    }

    #[test]
    fn plain_text_with_bare_ampersands_and_brackets_shows_as_it_is() {
        assert_eq!(
            sanitize("Tom & Jerry <3 > 2"),
            "Tom &amp; Jerry &lt;3 &gt; 2"
        );
        assert_eq!(
            sanitize("a &amp; b &#169; &#xA9;"),
            "a &amp; b &#169; &#xA9;"
        );
        assert_eq!(sanitize("&nbsp;"), "&amp;nbsp;");
    }

    #[test]
    fn tags_pango_cant_show_are_dropped_or_shown_as_text() {
        assert_eq!(sanitize("a<br/>b<img src=\"x.png\" alt=\"x\"/>c"), "abc");
        assert_eq!(
            sanitize("<span color='red'>x</span>"),
            "&lt;span color='red'&gt;x&lt;/span&gt;"
        );
        assert_eq!(sanitize("<a>no link</a>"), "&lt;a&gt;no link");
    }

    #[test]
    fn tags_left_open_or_crossed_are_closed() {
        assert_eq!(sanitize("<b>open"), "<b>open</b>");
        assert_eq!(sanitize("<b><i>x</b>y</i>"), "<b><i>x</i></b>y");
        assert_eq!(sanitize("x</b>"), "x");
    }
}
