// SPDX-FileCopyrightText: 2000, 2003 Red Hat, Inc.
// SPDX-FileCopyrightText: 2007-2008 Ryan Lortie <desrt@desrt.ca>
// SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
// SPDX-License-Identifier: GPL-3.0-or-later AND LGPL-2.1-or-later

//! A tokenizer that accepts exactly what GLib's gmarkup accepts (04).
//!
//! Marco parses themes with GMarkup, which is laxer than XML: an attribute value ends at
//! its closing quote whatever it contains, so the raw `version="< 3.1"` in most v3 themes
//! is fine, and attributes need no whitespace between them. Strict XML parsers reject
//! both, so this is a port of `gmarkup.c`'s state machine (GLib 2.88).

use std::collections::VecDeque;

/// One thing the tokenizer reports, as gmarkup's callbacks would.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    /// A start tag, with its attributes in document order.
    Start {
        name: String,
        attributes: Vec<(String, String)>,
    },
    /// An end tag, or the end of an empty-element tag.
    End { name: String },
    /// The text between two tags, unescaped. Only text inside the root element is
    /// reported, and a comment splits it in two.
    Text(String),
}

/// A document gmarkup would reject.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MarkupError {
    pub line: usize,
    pub message: String,
}

type Item = Result<(Event, usize), MarkupError>;

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// Outside the root element.
    Start,
    /// Inside an element, before any text.
    Text,
    /// Just after a `<`.
    AfterOpenAngle,
    Done,
}

/// Reports the events of a document one at a time, each with the line gmarkup reports
/// for it, so that errors name the lines Marco names. The first error ends the iteration.
pub(crate) struct Markup<'a> {
    text: &'a [u8],
    pos: usize,
    state: State,
    stack: Vec<String>,
    pending: VecDeque<Item>,
    document_empty: bool,
    line_pos: usize,
    line: usize,
}

impl<'a> Markup<'a> {
    pub(crate) fn new(text: &'a [u8]) -> Self {
        Markup {
            text,
            pos: 0,
            state: State::Start,
            stack: Vec::new(),
            pending: VecDeque::new(),
            document_empty: true,
            line_pos: 0,
            line: 1,
        }
    }

    /// The line of the byte at `pos`. Positions only move forwards, so this counts
    /// newlines from the last position asked about.
    fn line_at(&mut self, pos: usize) -> usize {
        let pos = pos.min(self.text.len());
        if pos < self.line_pos {
            self.line_pos = 0;
            self.line = 1;
        }
        self.line += self.text[self.line_pos..pos]
            .iter()
            .filter(|&&c| c == b'\n')
            .count();
        self.line_pos = pos;
        self.line
    }

    /// The line gmarkup reports with the parser at `pos`. gmarkup moves to the next line
    /// as it steps onto a newline, so a position on a newline is on the next line.
    fn reported_line(&mut self, pos: usize) -> usize {
        self.line_at(pos + 1)
    }

    fn error(&mut self, pos: usize, message: String) -> MarkupError {
        MarkupError {
            line: self.line_at(pos),
            message,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.pos).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(is_space) {
            self.pos += 1;
        }
    }

    fn advance_to_name_end(&mut self) -> &'a [u8] {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| !is_common_name_end(c) && !is_space(c))
        {
            self.pos += 1;
        }
        &self.text[start..self.pos]
    }

    fn current_element(&self) -> &str {
        self.stack.last().map_or("", String::as_str)
    }

    fn step(&mut self) -> Result<(), MarkupError> {
        match self.state {
            State::Done => Ok(()),
            State::Start => {
                self.skip_spaces();
                match self.peek() {
                    None => {
                        self.state = State::Done;
                        if self.document_empty {
                            return Err(self.error(
                                self.pos,
                                "Document was empty or contained only whitespace".to_owned(),
                            ));
                        }
                        Ok(())
                    }
                    Some(b'<') => {
                        self.pos += 1;
                        self.document_empty = false;
                        self.state = State::AfterOpenAngle;
                        Ok(())
                    }
                    Some(_) => Err(self.error(
                        self.pos,
                        "Document must begin with an element (e.g. <book>)".to_owned(),
                    )),
                }
            }
            State::Text => {
                let start = self.pos;
                let Some(offset) = self.text[start..].iter().position(|&c| c == b'<') else {
                    return Err(self.error(
                        self.text.len(),
                        format!(
                            "Document ended unexpectedly with elements still open — “{}” was \
                             the last element opened",
                            self.current_element()
                        ),
                    ));
                };
                let end = start + offset;
                let text = self.unescape(start, end, false)?;
                let line = self.line_at(end);
                self.pending.push_back(Ok((Event::Text(text), line)));
                self.pos = end + 1;
                self.state = State::AfterOpenAngle;
                Ok(())
            }
            State::AfterOpenAngle => self.after_open_angle(),
        }
    }

    fn after_state(&self) -> State {
        if self.stack.is_empty() {
            State::Start
        } else {
            State::Text
        }
    }

    fn after_open_angle(&mut self) -> Result<(), MarkupError> {
        match self.peek() {
            None => Err(self.error(
                self.pos,
                "Document ended unexpectedly just after an open angle bracket “<”".to_owned(),
            )),
            Some(b'?' | b'!') => self.passthrough(),
            Some(b'/') => {
                self.pos += 1;
                self.close_tag()
            }
            Some(c) if is_common_name_end(c) => Err(self.error(
                self.pos,
                format!(
                    "“{}” is not a valid character following a “<” character; it may not \
                     begin an element name",
                    char_at(self.text, self.pos)
                ),
            )),
            Some(_) => self.open_tag(),
        }
    }

    /// A comment, processing instruction, doctype or CDATA section, which Marco ignores.
    /// It ends at the first `>` that closes it by gmarkup's rules, which count nested
    /// angle brackets only for a doctype.
    fn passthrough(&mut self) -> Result<(), MarkupError> {
        let begin = self.pos - 1;
        let mut balance = 1i32;
        let mut i = self.pos;
        while i < self.text.len() {
            match self.text[i] {
                b'<' => balance += 1,
                b'>' => {
                    balance -= 1;
                    let chunk = &self.text[begin..i];
                    if (chunk[1] == b'?' && chunk.ends_with(b"?"))
                        || (chunk.starts_with(b"<!--") && chunk.ends_with(b"--"))
                        || (chunk.starts_with(b"<![CDATA[") && chunk.ends_with(b"]]"))
                        || (chunk.starts_with(b"<!DOCTYPE") && balance == 0)
                    {
                        self.pos = i + 1;
                        self.state = self.after_state();
                        return Ok(());
                    }
                }
                _ => {}
            }
            i += 1;
        }
        Err(self.error(
            self.text.len(),
            "Document ended unexpectedly inside a comment or processing instruction".to_owned(),
        ))
    }

    fn close_tag_ended(&mut self) -> MarkupError {
        let message = match self.stack.last() {
            Some(name) => {
                format!("Document ended unexpectedly inside the close tag for element “{name}”")
            }
            None => "Document ended unexpectedly inside the close tag for an unopened element"
                .to_owned(),
        };
        self.error(self.text.len(), message)
    }

    fn close_tag(&mut self) -> Result<(), MarkupError> {
        match self.peek() {
            None => return Err(self.close_tag_ended()),
            Some(c) if is_common_name_end(c) => {
                let c = char_at(self.text, self.pos);
                return Err(self.error(
                    self.pos,
                    format!(
                        "“{c}” is not a valid character following the characters “</”; “{c}” \
                         may not begin an element name"
                    ),
                ));
            }
            Some(_) => {}
        }
        let name = String::from_utf8_lossy(self.advance_to_name_end()).into_owned();
        self.skip_spaces();
        match self.peek() {
            None => Err(self.close_tag_ended()),
            Some(b'>') => {
                if self.stack.is_empty() {
                    return Err(self.error(
                        self.pos,
                        format!("Element “{name}” was closed, no element is currently open"),
                    ));
                }
                if name != self.current_element() {
                    let message = format!(
                        "Element “{name}” was closed, but the currently open element is “{}”",
                        self.current_element()
                    );
                    return Err(self.error(self.pos, message));
                }
                let line = self.reported_line(self.pos + 1);
                self.stack.pop();
                self.pos += 1;
                self.state = self.after_state();
                self.pending.push_back(Ok((Event::End { name }, line)));
                Ok(())
            }
            Some(_) => Err(self.error(
                self.pos,
                format!(
                    "“{}” is not a valid character following the close element name “{name}”; \
                     the allowed character is “>”",
                    char_at(self.text, self.pos)
                ),
            )),
        }
    }

    fn open_tag(&mut self) -> Result<(), MarkupError> {
        let name = self.advance_to_name_end();
        if self.peek().is_none() {
            return Err(self.error(
                self.pos,
                "Document ended unexpectedly inside an element name".to_owned(),
            ));
        }
        let name = String::from_utf8_lossy(name).into_owned();
        let mut attributes = Vec::new();
        loop {
            self.skip_spaces();
            match self.peek() {
                None => {
                    return Err(self.error(
                        self.pos,
                        "Document ended unexpectedly inside an element-opening tag.".to_owned(),
                    ));
                }
                Some(b'/') => {
                    self.pos += 1;
                    let line = self.reported_line(self.pos);
                    let start = self.start_event(name.clone(), attributes, line)?;
                    self.pending.push_back(Ok(start));
                    match self.peek() {
                        Some(b'>') => {
                            self.pos += 1;
                            self.state = self.after_state();
                            let line = self.reported_line(self.pos);
                            self.pending.push_back(Ok((Event::End { name }, line)));
                            return Ok(());
                        }
                        None => {
                            return Err(self.error(
                                self.pos,
                                format!(
                                    "Document ended unexpectedly, expected to see a close angle \
                                     bracket ending the tag <{name}/>"
                                ),
                            ));
                        }
                        Some(_) => {
                            return Err(self.error(
                                self.pos,
                                format!(
                                    "Odd character “{}”, expected a “>” character to end the \
                                     empty-element tag “{name}”",
                                    char_at(self.text, self.pos)
                                ),
                            ));
                        }
                    }
                }
                Some(b'>') => {
                    let line = self.reported_line(self.pos + 1);
                    let start = self.start_event(name.clone(), attributes, line)?;
                    self.pending.push_back(Ok(start));
                    self.stack.push(name);
                    self.pos += 1;
                    self.state = State::Text;
                    return Ok(());
                }
                Some(c) if !is_common_name_end(c) => {
                    let attribute = self.attribute(&name)?;
                    if attributes.len() >= 1000 {
                        return Err(self
                            .error(self.pos, format!("Too many attributes in element “{name}”")));
                    }
                    attributes.push(attribute);
                }
                Some(_) => {
                    return Err(self.error(
                        self.pos,
                        format!(
                            "Odd character “{}”, expected a “>” or “/” character to end the \
                             start tag of element “{name}”, or optionally an attribute; perhaps \
                             you used an invalid character in an attribute name",
                            char_at(self.text, self.pos)
                        ),
                    ));
                }
            }
        }
    }

    /// gmarkup validates the element name only once the start tag is complete, after its
    /// attributes.
    fn start_event(
        &mut self,
        name: String,
        attributes: Vec<(String, String)>,
        line: usize,
    ) -> Result<(Event, usize), MarkupError> {
        if let Err(message) = validate_name(name.as_bytes()) {
            return Err(MarkupError { line, message });
        }
        Ok((Event::Start { name, attributes }, line))
    }

    fn attribute(&mut self, element: &str) -> Result<(String, String), MarkupError> {
        let name = self.advance_to_name_end();
        self.skip_spaces();
        if self.peek().is_none() {
            return Err(self.error(
                self.pos,
                "Document ended unexpectedly inside an attribute name".to_owned(),
            ));
        }
        if let Err(message) = validate_name(name) {
            return Err(self.error(self.pos, message));
        }
        let name = String::from_utf8_lossy(name).into_owned();
        if self.peek() != Some(b'=') {
            return Err(self.error(
                self.pos,
                format!(
                    "Odd character “{}”, expected a “=” after attribute name “{name}” of \
                     element “{element}”",
                    char_at(self.text, self.pos)
                ),
            ));
        }
        self.pos += 1;
        self.skip_spaces();
        let quote = match self.peek() {
            None => {
                return Err(self.error(
                    self.pos,
                    "Document ended unexpectedly after the equals sign following an attribute \
                     name; no attribute value"
                        .to_owned(),
                ));
            }
            Some(quote @ (b'"' | b'\'')) => quote,
            Some(_) => {
                return Err(self.error(
                    self.pos,
                    format!(
                        "Odd character “{}”, expected an open quote mark after the equals sign \
                         when giving value for attribute “{name}” of element “{element}”",
                        char_at(self.text, self.pos)
                    ),
                ));
            }
        };
        let start = self.pos + 1;
        let Some(offset) = self.text[start..].iter().position(|&c| c == quote) else {
            return Err(self.error(
                self.text.len(),
                "Document ended unexpectedly while inside an attribute value".to_owned(),
            ));
        };
        let end = start + offset;
        let value = self.unescape(start, end, true)?;
        self.pos = end + 1;
        Ok((name, value))
    }

    /// gmarkup's `unescape_gstring_inplace`: resolves the five entities and character
    /// references, and normalizes line breaks, which in an attribute value also turns
    /// tabs and newlines into spaces.
    fn unescape(
        &mut self,
        start: usize,
        end: usize,
        attribute: bool,
    ) -> Result<String, MarkupError> {
        let text = &self.text[start..end];
        let mut out = Vec::with_capacity(text.len());
        let mut i = 0;
        while i < text.len() {
            let c = text[i];
            match c {
                b'\t' | b'\n' if attribute => out.push(b' '),
                b'\r' => {
                    out.push(if attribute { b' ' } else { b'\n' });
                    if text.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                }
                b'&' => {
                    let rest = &text[i + 1..];
                    match unescape_entity(rest) {
                        Ok((replacement, used)) => {
                            let mut buf = [0; 4];
                            out.extend_from_slice(replacement.encode_utf8(&mut buf).as_bytes());
                            i += used;
                        }
                        Err(message) => return Err(self.error(start + i, message)),
                    }
                }
                _ => out.push(c),
            }
            i += 1;
        }
        String::from_utf8(out).map_err(|_| {
            self.error(
                start,
                "Invalid UTF-8 encoded text in name — not valid".to_owned(),
            )
        })
    }
}

impl Iterator for Markup<'_> {
    type Item = Item;

    fn next(&mut self) -> Option<Item> {
        loop {
            if let Some(item) = self.pending.pop_front() {
                if item.is_err() {
                    self.pending.clear();
                    self.state = State::Done;
                }
                return Some(item);
            }
            if self.state == State::Done {
                return None;
            }
            if let Err(error) = self.step() {
                self.state = State::Done;
                self.pending.push_back(Err(error));
            }
        }
    }
}

/// The entity after an `&`, and how many bytes of `rest` it used.
fn unescape_entity(rest: &[u8]) -> Result<(char, usize), String> {
    const ENTITIES: [(&[u8], char); 5] = [
        (b"lt;", '<'),
        (b"gt;", '>'),
        (b"amp;", '&'),
        (b"quot;", '"'),
        (b"apos;", '\''),
    ];
    if let Some(reference) = rest.strip_prefix(b"#") {
        let (digits, radix, skip) = match reference.strip_prefix(b"x") {
            Some(hex) => (hex, 16, 2),
            None => (reference, 10, 1),
        };
        let len = digits
            .iter()
            .take_while(|&&c| (c as char).is_digit(radix))
            .count();
        let value = std::str::from_utf8(&digits[..len])
            .ok()
            .and_then(|digits| u32::from_str_radix(digits, radix).ok());
        let Some(value) = value else {
            return Err(format!(
                "Failed to parse “{}”, which should have been a digit inside a character \
                 reference (&#234; for example) — perhaps the digit is too large",
                String::from_utf8_lossy(&digits[..len])
            ));
        };
        if digits.get(len) != Some(&b';') {
            return Err(
                "Character reference did not end with a semicolon; most likely you \
                        used an ampersand character without intending to start an entity — \
                        escape ampersand as &amp;"
                    .to_owned(),
            );
        }
        let permitted = (1..=0xD7FF).contains(&value)
            || (0xE000..=0xFFFD).contains(&value)
            || (0x10000..=0x10FFFF).contains(&value);
        return match char::from_u32(value).filter(|_| permitted) {
            Some(c) => Ok((c, skip + len + 1)),
            None => Err(format!(
                "Character reference “{}” does not encode a permitted character",
                String::from_utf8_lossy(&digits[..len])
            )),
        };
    }
    for (name, c) in ENTITIES {
        if rest.starts_with(name) {
            return Ok((c, name.len()));
        }
    }
    if rest.first() == Some(&b';') {
        return Err(
            "Empty entity “&;” seen; valid entities are: &amp; &quot; &lt; &gt; &apos;".to_owned(),
        );
    }
    match rest.iter().position(|&c| c == b';') {
        Some(end) => Err(format!(
            "Entity name “{}” is not known",
            String::from_utf8_lossy(&rest[..end])
        )),
        None => Err(
            "Entity did not end with a semicolon; most likely you used an ampersand \
                     character without intending to start an entity — escape ampersand as &amp;"
                .to_owned(),
        ),
    }
}

/// gmarkup's `name_validate`, for element and attribute names.
fn validate_name(name: &[u8]) -> Result<(), String> {
    let invalid = || format!("“{}” is not a valid name", String::from_utf8_lossy(name));
    let Ok(text) = std::str::from_utf8(name) else {
        return Err(format!(
            "Invalid UTF-8 encoded text in name — not valid “{}”",
            String::from_utf8_lossy(name)
        ));
    };
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return Err(invalid());
    };
    if !(first.is_ascii_alphabetic()
        || first == '_'
        || first == ':'
        || (!first.is_ascii() && first.is_alphabetic()))
    {
        return Err(invalid());
    }
    for c in chars {
        let ok = c.is_ascii_alphanumeric()
            || matches!(c, '.' | '-' | '_' | ':')
            || (!c.is_ascii() && c.is_alphabetic());
        if !ok {
            return Err(format!("{}: “{c}”", invalid()));
        }
    }
    Ok(())
}

/// gmarkup's `IS_COMMON_NAME_END_CHAR`.
fn is_common_name_end(c: u8) -> bool {
    matches!(c, b'=' | b'/' | b'>' | b' ')
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

/// The character at `pos`, for error messages.
fn char_at(text: &[u8], pos: usize) -> String {
    let end = (pos + 4).min(text.len());
    match String::from_utf8_lossy(&text[pos..end]).chars().next() {
        Some(c) => c.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(text: &str) -> Result<Vec<Event>, MarkupError> {
        Markup::new(text.as_bytes())
            .map(|event| event.map(|(event, _)| event))
            .collect()
    }

    fn start(name: &str, attributes: &[(&str, &str)]) -> Event {
        Event::Start {
            name: name.to_owned(),
            attributes: attributes
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
        }
    }

    fn end(name: &str) -> Event {
        Event::End {
            name: name.to_owned(),
        }
    }

    #[test]
    fn a_raw_open_angle_in_an_attribute_value_is_part_of_the_value() {
        // Menta, BlackMATE, Arc and Mint-Y all write this.
        assert_eq!(
            events(r#"<frame_geometry version="< 3.1" name="a"/>"#).unwrap(),
            [
                start("frame_geometry", &[("version", "< 3.1"), ("name", "a")]),
                end("frame_geometry"),
            ]
        );
    }

    #[test]
    fn attributes_need_no_whitespace_between_them() {
        // BlackMATE's metacity-theme-3.xml:1669.
        assert_eq!(
            events(r#"<frame version=">= 3.3"focus="yes"/>"#).unwrap(),
            [
                start("frame", &[("version", ">= 3.3"), ("focus", "yes")]),
                end("frame"),
            ]
        );
    }

    #[test]
    fn a_raw_close_angle_and_the_other_quote_are_part_of_a_value() {
        assert_eq!(
            events(r#"<a b='x > "y"' c="it's"></a>"#).unwrap(),
            [
                start("a", &[("b", r#"x > "y""#), ("c", "it's")]),
                Event::Text(String::new()),
                end("a"),
            ]
        );
    }

    #[test]
    fn entities_and_character_references_are_unescaped() {
        assert_eq!(
            events(r#"<a b="&lt;&gt;&amp;&quot;&apos;&#65;&#x42;">1 &lt; 2</a>"#).unwrap(),
            [
                start("a", &[("b", "<>&\"'AB")]),
                Event::Text("1 < 2".to_owned()),
                end("a"),
            ]
        );
    }

    #[test]
    fn whitespace_in_attribute_values_is_normalized_to_spaces() {
        assert_eq!(
            events("<a b=\"x\ty\r\nz\"/>").unwrap(),
            [start("a", &[("b", "x y z")]), end("a")]
        );
    }

    #[test]
    fn spaces_are_allowed_around_the_equals_sign() {
        assert_eq!(
            events("<a b = \"1\"\n\tc\n=\n'2' />").unwrap(),
            [start("a", &[("b", "1"), ("c", "2")]), end("a")]
        );
    }

    #[test]
    fn comments_processing_instructions_and_doctypes_are_skipped() {
        let text = "<?xml version=\"1.0\"?>\n<!DOCTYPE t [<!ENTITY x \"y\">]>\n\
                    <!-- a <b> comment -->\n<t><!-- x -->text</t>\n";
        assert_eq!(
            events(text).unwrap(),
            [
                start("t", &[]),
                Event::Text(String::new()),
                Event::Text("text".to_owned()),
                end("t"),
            ]
        );
    }

    #[test]
    fn text_is_split_where_a_comment_interrupts_it() {
        assert_eq!(
            events("<t>a<!-- -->b</t>").unwrap(),
            [
                start("t", &[]),
                Event::Text("a".to_owned()),
                Event::Text("b".to_owned()),
                end("t"),
            ]
        );
    }

    #[test]
    fn events_carry_the_line_gmarkup_reports() {
        let lines: Vec<usize> = Markup::new(b"<a>\n<b\n c='1'/>\n</a>")
            .map(|event| event.unwrap().1)
            .collect();
        // gmarkup reports a tag from just past its end, which for <a> and <b/>'s end is
        // already the next line. Then the text before <b>, <b/>'s start at its '>', the
        // text before </a>, and </a>.
        assert_eq!(lines, [2, 2, 3, 4, 4, 4]);
    }

    fn error(text: &str) -> MarkupError {
        events(text).unwrap_err()
    }

    #[test]
    fn a_mismatched_close_tag_is_an_error() {
        let e = error("<a>\n</b>");
        assert_eq!(e.line, 2);
        assert!(e.message.contains("“b” was closed"), "{}", e.message);
    }

    #[test]
    fn an_unknown_entity_is_an_error() {
        assert!(error("<a b='&nbsp;'/>").message.contains("not known"));
        assert!(error("<a>&</a>").message.contains("semicolon"));
    }

    #[test]
    fn an_unquoted_attribute_value_is_an_error() {
        assert!(error("<a b=1/>").message.contains("open quote"));
    }

    #[test]
    fn an_attribute_without_a_value_is_an_error() {
        assert!(error("<a b/>").message.contains("expected a “=”"));
    }

    #[test]
    fn text_outside_the_root_is_an_error() {
        assert!(
            error("x<a/>")
                .message
                .contains("must begin with an element")
        );
        assert!(
            error("<a/>x")
                .message
                .contains("must begin with an element")
        );
    }

    #[test]
    fn an_unfinished_document_is_an_error() {
        assert!(error("<a>").message.contains("still open"));
        assert!(error("<a b='1").message.contains("attribute value"));
        assert!(error("<!-- x").message.contains("comment"));
        assert!(error("  ").message.contains("empty"));
    }

    #[test]
    fn an_invalid_element_name_is_an_error() {
        assert!(error("<1a/>").message.contains("not a valid name"));
    }
}
