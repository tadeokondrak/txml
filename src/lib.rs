//! # txml
//!
//! An XML parser. It's small, but it:
//!
//! - Doesn't parse or validate DTDs
//! - Doesn't support custom entities
//! - Requires the full document to be loaded in memory
//! - Accepts some non-well-formed documents
//! - Doesn't have any dependencies
//! - Doesn't allocate
//!
//! This parser is not meant for usecases where you'd like good error messages
//! or perfect XML compliance. It's best used when communicating with a known
//! system, or when parsing existing, known documents written by hand.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/txml/0.3.0")]

#[cfg(test)]
mod tests;

use core::convert::TryInto;
use core::fmt::{self, Debug, Display, Write};

const WHITESPACE: &[char] = &[' ', '\t', '\r', '\n'];
const WHITESPACE_AND_RANGLE_AND_SLASH: &[char] = &[' ', '\t', '\r', '\n', '>', '/'];

/// An XML event.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Event<'a> {
    /// An opening tag of an XML element.
    Open(
        /// Tag name.
        &'a str,
        /// Attributes.
        Attrs<'a>,
    ),
    /// A closing tag of an XML element.
    /// This is also emitted after self-closing tags.
    Close(
        /// Tag name.
        &'a str,
    ),
    /// A doctype declaration.
    Doctype(
        /// Doctype name.
        &'a str,
        /// Doctype body. Can be empty.
        &'a str,
    ),
    /// A processing instruction.
    Pi(
        /// Processing instruction content.
        &'a str,
    ),
    /// A comment.
    Comment(
        /// Comment content.
        &'a str,
    ),
    /// Character data.
    Text(
        /// Text content.
        Text<'a>,
    ),
}

/// An iterator over XML attributes.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Attrs<'a> {
    // invariant: no trailing whitespace
    text: &'a str,
}

/// A parsing error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub &'static str);

impl Error {
    /// '=' character not found in attribute
    pub const ATTR_MISSING_EQ: Error = Error("'=' character not found in attribute");
    /// quote character not found in attribute
    pub const ATTR_MISSING_QUOTE: Error = Error("quote character not found in attribute");
    /// end quote character not found in attribute
    pub const ATTR_MISSING_END_QUOTE: Error = Error("end quote character not found in attribute");
    /// invalid quote character in attribute
    pub const ATTR_INVALID_QUOTE: Error = Error("invalid quote character in attribute");
    /// unterminated entity (missing ';')
    pub const UNTERMINATED_ENTITY: Error = Error("unterminated entity (missing ';')");
    /// invalid named entity
    pub const INVALID_NAMED_ENTITY: Error = Error("invalid named entity");
    /// invalid numeric entity
    pub const INVALID_NUMERIC_ENTITY: Error = Error("invalid numeric entity");
    /// unterminated comment (missing '-->')
    pub const UNTERMINATED_COMMENT: Error = Error("unterminated comment (missing '-->')");
    /// unterminated PI (missing '?>')
    pub const UNTERMINATED_PI: Error = Error("unterminated PI (missing '?>')");
    /// unterminated CDATA section (missing ']]>')
    pub const UNTERMINATED_CDATA: Error = Error("unterminated CDATA section (missing ']]>')");
    /// unterminated doctype declaration (missing '>')
    pub const UNTERMINATED_DOCTYPE: Error = Error("unterminated doctype declaration (missing '>')");
    /// unterminated doctype subset (missing ']')
    pub const UNTERMINATED_DOCTYPE_SUBSET: Error =
        Error("unterminated doctype subset (missing ']')");
    /// unterminated tag (missing '>')
    pub const UNTERMINATED_TAG: Error = Error("unterminated tag (missing '>')");
    /// unterminated closing tag (missing '>')
    pub const UNTERMINATED_CLOSING_TAG: Error = Error("unterminated closing tag (missing '>')");
    /// invalid tag name
    pub const INVALID_TAG_NAME: Error = Error("invalid tag name");
}

impl<'a> Attrs<'a> {
    /// Iterates through the attributes and returns the value for the given
    /// attribute name, if present.
    pub fn get(&self, name: &str) -> Result<Option<Text<'a>>, Error> {
        for kv in self.clone() {
            let (key, value) = kv?;
            if name == key {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
}

impl<'a> Iterator for Attrs<'a> {
    type Item = Result<(&'a str, Text<'a>), Error>;

    fn next(&mut self) -> Option<Result<(&'a str, Text<'a>), Error>> {
        if self.text.is_empty() {
            // self.text invariant: no trailing whitespace
            return None;
        }
        let Some(eq) = self.text.find('=') else {
            self.text = "";
            return Some(Err(Error::ATTR_MISSING_EQ));
        };
        let (start, rest) = self.text.split_at(eq);
        let start = start.trim_matches(WHITESPACE);
        let rest = rest[1..].trim_start_matches(WHITESPACE);
        let mut it = rest.char_indices();
        let Some((_, quote)) = it.next() else {
            self.text = "";
            return Some(Err(Error::ATTR_MISSING_QUOTE));
        };
        if quote != '\'' && quote != '"' {
            self.text = "";
            return Some(Err(Error::ATTR_INVALID_QUOTE));
        }
        let val_end = loop {
            match it.next() {
                Some((i, c)) if c == quote => break i,
                Some((_, _)) => {}
                None => {
                    self.text = "";
                    return Some(Err(Error::ATTR_MISSING_END_QUOTE));
                }
            }
        };
        self.text = it.as_str();
        Some(Ok((start, Text::Escaped(&rest[1..val_end]))))
    }
}

/// A string that can contain XML entity references.
///
/// This type is an iterator of characters.
/// To convert to a string, use the Display impl.
/// To compare equality, use the PartialEq\<str\> impl.
#[derive(Clone, Eq, Debug)]
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
            Text::Escaped(s) => Text::Escaped(s)
                .map(|c| f.write_char(c.unwrap_or('�')))
                .collect(),
        }
    }
}

impl<'a> Iterator for Text<'a> {
    type Item = Result<char, Error>;

    fn next(&mut self) -> Option<Result<char, Error>> {
        match *self {
            Text::Verbatim(ref mut s) => {
                let mut it = s.chars();
                let c = it.next()?;
                *s = it.as_str();
                Some(Ok(c))
            }
            Text::Escaped(ref mut s) => {
                if s.starts_with('&') {
                    let Some(semi) = s.find(';') else {
                        *s = "";
                        return Some(Err(Error::UNTERMINATED_ENTITY));
                    };
                    let esc = &s[1..semi];
                    *s = &s[semi + 1..];
                    match esc {
                        "lt" => Some(Ok('<')),
                        "gt" => Some(Ok('>')),
                        "amp" => Some(Ok('&')),
                        "apos" => Some(Ok('\'')),
                        "quot" => Some(Ok('"')),
                        esc if esc.starts_with('#') => {
                            let (esc, radix) = match esc[1..].strip_prefix('x') {
                                Some(esc) => (esc, 16),
                                None => (&esc[1..], 10),
                            };
                            match u32::from_str_radix(esc, radix)
                                .ok()
                                .and_then(|n| n.try_into().ok())
                            {
                                Some(c) => Some(Ok(c)),
                                None => {
                                    *s = "";
                                    return Some(Err(Error::INVALID_NUMERIC_ENTITY));
                                }
                            }
                        }
                        _ => {
                            *s = "";
                            return Some(Err(Error::INVALID_NAMED_ENTITY));
                        }
                    }
                } else {
                    let mut it = s.chars();
                    let c = it.next()?;
                    *s = it.as_str();
                    Some(Ok(c))
                }
            }
        }
    }
}

impl<'a> PartialEq for Text<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.clone().eq(other.clone())
    }
}

impl<'a> PartialEq<str> for Text<'a> {
    fn eq(&self, other: &str) -> bool {
        self.clone().eq(other.chars().map(Ok))
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
    type Item = Result<Event<'a>, Error>;

    fn next(&mut self) -> Option<Result<Event<'a>, Error>> {
        if let Some(tag) = self.self_closing.take() {
            Some(Ok(Event::Close(tag)))
        } else if self.consume("<?") {
            let Some(text) = self.consume_to("?>") else {
                self.doc = "";
                return Some(Err(Error::UNTERMINATED_PI));
            };
            Some(Ok(Event::Pi(text)))
        } else if self.consume("<!DOCTYPE") {
            let Some(i) = self.doc.find(&['[', '>']) else {
                self.doc = "";
                return Some(Err(Error::UNTERMINATED_DOCTYPE));
            };
            if self.doc[i..].starts_with("[") {
                let name = self.doc[..i].trim_matches(WHITESPACE);
                self.doc = &self.doc[i + 1..];
                let Some(body) = self.consume_to("]") else {
                    self.doc = "";
                    return Some(Err(Error::UNTERMINATED_DOCTYPE_SUBSET));
                };
                let body = body.trim_matches(WHITESPACE);
                let Some(_ws) = self.consume_to(">") else {
                    self.doc = "";
                    return Some(Err(Error::UNTERMINATED_DOCTYPE));
                };
                Some(Ok(Event::Doctype(name, body)))
            } else {
                Some(Ok(Event::Doctype(
                    self.consume_to(">")
                        .unwrap_or_else(|| unreachable!())
                        .trim_matches(WHITESPACE),
                    "",
                )))
            }
        } else if self.consume("<!--") {
            let Some(text) = self.consume_to("-->") else {
                self.doc = "";
                return Some(Err(Error::UNTERMINATED_COMMENT));
            };
            Some(Ok(Event::Comment(text)))
        } else if self.consume("<![CDATA[") {
            let Some(text) = self.consume_to("]]>") else {
                self.doc = "";
                return Some(Err(Error::UNTERMINATED_CDATA));
            };
            Some(Ok(Event::Text(Text::Verbatim(text))))
        } else if self.consume("</") {
            let Some(tag_name) = self.consume_to(">") else {
                self.doc = "";
                return Some(Err(Error::UNTERMINATED_CLOSING_TAG));
            };
            let tag_name = tag_name.trim_matches(WHITESPACE);
            Some(Ok(Event::Close(tag_name)))
        } else if self.consume("<") {
            let Some(i) = self.doc.find(WHITESPACE_AND_RANGLE_AND_SLASH) else {
                self.doc = "";
                return Some(Err(Error::UNTERMINATED_TAG));
            };
            let tag = self.doc[..i].trim_matches(WHITESPACE);
            if tag == "" {
                self.doc = "";
                return Some(Err(Error::INVALID_TAG_NAME));
            }
            self.doc = &self.doc[i..];
            let Some(attrs) = self.consume_to(">") else {
                self.doc = "";
                return Some(Err(Error::UNTERMINATED_TAG));
            };
            let mut attrs = attrs.trim_matches(WHITESPACE);
            if attrs.ends_with('/') {
                self.self_closing = Some(tag);
                attrs = attrs[..attrs.len() - 1].trim_end_matches(WHITESPACE);
            }
            Some(Ok(Event::Open(tag, Attrs { text: attrs })))
        } else if self.doc.starts_with("&") {
            if let Some(i) = self.doc.find([';']) {
                let ret = &self.doc[..=i];
                self.doc = &self.doc[i + 1..];
                Some(Ok(Event::Text(Text::Escaped(ret))))
            } else {
                let i = self.doc.find(['<']).unwrap_or_else(|| self.doc.len());
                let ret = &self.doc[..i];
                self.doc = &self.doc[i..];
                Some(Ok(Event::Text(Text::Escaped(ret))))
            }
        } else if !self.doc.is_empty() {
            let i = self.doc.find(['<', '&']).unwrap_or_else(|| self.doc.len());
            let ret = &self.doc[..i];
            self.doc = &self.doc[i..];
            Some(Ok(Event::Text(Text::Verbatim(ret))))
        } else {
            None
        }
    }
}
