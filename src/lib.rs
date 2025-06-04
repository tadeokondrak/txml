//! # txml
//!
//! A no_std non-conforming, non-validating, non-streaming, zero-dependency,
//! and zero-allocation XML parser in about 200 lines of safe Rust.
//!
//! It handles most sane XML files including those with ampersand escapes.
//! It has no error information other than returning None.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/txml/0.1.0")]

use core::convert::TryInto;
use core::fmt::{self, Debug, Display, Write};

const WHITESPACE: &[char] = &[' ', '\t', '\r', '\n'];
const WHITESPACE_AND_RANGLE_AND_SLASH: &[char] = &[' ', '\t', '\r', '\n', '>', '/'];

/// An XML event.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Event<'a> {
    /// An opening tag of an XML element.
    Open(&'a str, Attrs<'a>),
    /// A closing tag of an XML element.
    /// This is also emitted after self-closing tags.
    Close(&'a str),
    /// A processing instruction.
    Pi(&'a str),
    /// A comment.
    Comment(&'a str),
    /// Character data.
    Text(Text<'a>),
}

/// An iterator over XML attributes.
#[derive(Clone, Eq, PartialEq)]
pub struct Attrs<'a> {
    text: &'a str,
}

impl<'a> Attrs<'a> {
    /// Iterates through the attributes and returns the value for the given
    /// attribute name, if present.
    pub fn get(&self, name: &str) -> Option<Text<'a>> {
        for (key, value) in self.clone() {
            if name == key {
                return Some(value);
            }
        }
        None
    }
}

impl<'a> Debug for Attrs<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map().entries(self.clone()).finish()
    }
}

impl<'a> Iterator for Attrs<'a> {
    type Item = (&'a str, Text<'a>);

    fn next(&mut self) -> Option<(&'a str, Text<'a>)> {
        let eq = self.text.find('=')?;
        let (start, rest) = self.text.split_at(eq);
        let start = start.trim_matches(WHITESPACE);
        let rest = rest[1..].trim_start_matches(WHITESPACE);
        let mut it = rest.char_indices();
        let quote = it.next()?.1;
        if quote != '\'' && quote != '"' {
            return None;
        }
        let val_end = loop {
            match it.next() {
                Some((i, c)) if c == quote => break i,
                Some((_, _)) => {}
                None => return None,
            }
        };
        self.text = it.as_str();
        Some((start, Text::Escaped(&rest[1..val_end])))
    }
}

/// A string that can contain XML entity references.
///
/// This type is an iterator of characters.  
/// To convert to a string, use the Display impl.  
/// To compare equality, use the PartialEq\<str\> impl.
#[derive(Clone, Eq)]
pub enum Text<'a> {
    /// Text interpreted as-is, without any replacements.
    Verbatim(&'a str),
    /// Text interpreted with XML entity references.
    Escaped(&'a str),
}

impl<'a> Display for Text<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Text::Verbatim(s) => f.write_str(s),
            Text::Escaped(s) => Text::Escaped(s).map(|c| f.write_char(c)).collect(),
        }
    }
}

impl<'a> Debug for Text<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char('"')?;
        for c in self.clone() {
            for c in c.escape_debug() {
                f.write_char(c)?;
            }
        }
        f.write_char('"')
    }
}

impl<'a> Iterator for Text<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match *self {
            Text::Verbatim(ref mut s) => {
                let mut it = s.chars();
                let c = it.next()?;
                *s = it.as_str();
                Some(c)
            }
            Text::Escaped(ref mut s) => {
                if s.starts_with('&') {
                    let semi = s.find(';')?;
                    let esc = &s[1..semi];
                    *s = &s[semi + 1..];
                    match esc {
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "amp" => Some('&'),
                        "apos" => Some('\''),
                        "quot" => Some('"'),
                        s if s.starts_with("#x") => {
                            let n = u32::from_str_radix(&esc[2..], 16).ok()?;
                            n.try_into().ok()
                        }
                        s if s.starts_with('#') => {
                            let n = u32::from_str_radix(&esc[1..], 10).ok()?;
                            n.try_into().ok()
                        }
                        _ => None,
                    }
                } else {
                    let mut it = s.chars();
                    let c = it.next()?;
                    *s = it.as_str();
                    Some(c)
                }
            }
        }
    }
}

impl<'a> PartialEq for Text<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.clone().zip(other.clone()).all(|(a, b)| a == b)
    }
}

impl<'a> PartialEq<str> for Text<'a> {
    fn eq(&self, other: &str) -> bool {
        self.clone().zip(other.chars()).all(|(a, b)| a == b)
    }
}

impl<'a, 'b> PartialEq<&'b str> for Text<'a> {
    fn eq(&self, other: &&'b str) -> bool {
        self.eq(*other)
    }
}

/// An iterator over XML events.
pub struct Parser<'a> {
    doc: &'a str,
    self_closing: Option<&'a str>,
}

impl<'a> Parser<'a> {
    /// Creates a new parser.
    pub fn new(doc: &'a str) -> Self {
        Parser {
            doc,
            self_closing: None,
        }
    }

    fn consume(&mut self, pattern: &str) -> bool {
        if self.doc.starts_with(pattern) {
            self.doc = &self.doc[pattern.len()..];
            true
        } else {
            false
        }
    }

    fn consume_to(&mut self, pattern: &str) -> Option<&'a str> {
        let i = self.doc.find(pattern)?;
        let ret = &self.doc[0..i];
        self.doc = &self.doc[i + pattern.len()..];
        Some(ret)
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Event<'a>> {
        if let Some(tag) = self.self_closing.take() {
            Some(Event::Close(tag))
        } else if self.consume("<?") {
            Some(Event::Pi(self.consume_to("?>")?))
        } else if self.consume("<!--") {
            Some(Event::Comment(self.consume_to("-->")?))
        } else if self.consume("<![CDATA[") {
            Some(Event::Text(Text::Verbatim(self.consume_to("]]>")?)))
        } else if self.consume("</") {
            Some(Event::Close(self.consume_to(">")?.trim_matches(WHITESPACE)))
        } else if self.consume("<") {
            let i = self.doc.find(WHITESPACE_AND_RANGLE_AND_SLASH)?;
            let tag = self.doc[..i].trim_matches(WHITESPACE);
            self.doc = &self.doc[i..];
            let mut attrs = self.consume_to(">")?.trim_matches(WHITESPACE);
            if attrs.ends_with('/') {
                self.self_closing = Some(tag);
                attrs = &attrs[..attrs.len() - 1];
            }
            Some(Event::Open(tag, Attrs { text: attrs }))
        } else if !self.doc.is_empty() {
            let i = self.doc.find('<').unwrap_or_else(|| self.doc.len());
            let ret = &self.doc[..i];
            self.doc = &self.doc[i..];
            Some(Event::Text(Text::Escaped(ret)))
        } else {
            None
        }
    }
}
