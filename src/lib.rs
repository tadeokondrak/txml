//! # txml
//!
//! An XML parser. It's small, but it:
//!
//! - Doesn't parse or validate DTDs
//! - Doesn't expand custom entities
//! - Doesn't provide position information for errors.
//! - Requires the full document to be loaded in memory
//! - Accepts some non-well-formed documents
//! - Supports XML built-in entities like &amp;
//! - Doesn't have any dependencies
//! - Doesn't allocate
//!
//! This parser is not good for usecases where you need good error messages or
//! complex XML features. It's best used when communicating with a known
//! system, or when parsing existing, known documents written by hand.
//!
//! txml doesn't check for certain constructs that you may want to check for
//! at a higher level. These include:
//!
//! - Attribute names: TODO name characters and repeated names
//! - Tag names: txml doesn't verify that tag names match
//! `[a-zA-Z_:][-a-zA-Z0-9_:.]*`. You can do this yourself, if necessary.
//! - Entities: [`Text`]'s expansion will fail if custom entities are present.
//! You can reimplement expansion of [`Text`] if you need custom entities.
//! - DTDs: [`Event::Doctype`] does not parse the contents of the inline subset.
//!The contents are provided in case you want to parse themyourself.
//! - Namespaces: txml doesn't understand namespaces, but that doesn't preclude
//! implementing namespace awareness on top.
//! - Comments: XML doesn't allow `--` in comments. You can check this yourself.
//! - Text: XML doesn't allow `]]>` in text content (not attributes).
//! You can check this yourself.
//! - Invalid nesting: TODO
//!
//! Also note that txml requires you to actually process text data if you want to see all errors within it.
//! TODO

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/txml/0.3.0")]

#[cfg(test)]
mod tests;

use core::convert::TryInto as _;

const WHITESPACE: &[char] = &[' ', '\t', '\r', '\n'];

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
pub enum Error {
    /// invalid attribute name
    AttrInvalidName,
    /// '=' character not found in attribute
    AttrMissingEq,
    /// quote character not found in attribute
    AttrMissingQuote,
    /// end quote character not found in attribute
    AttrMissingEndQuote,
    /// invalid quote character in attribute
    AttrInvalidQuote,
    /// unterminated entity (missing ';')
    UnterminatedEntity,
    /// invalid named entity
    InvalidNamedEntity,
    /// invalid numeric entity
    InvalidNumericEntity,
    /// unterminated comment (missing '-->')
    UnterminatedComment,
    /// unterminated PI (missing '?>')
    UnterminatedPi,
    /// unterminated CDATA section (missing ']]>')
    UnterminatedCdata,
    /// unterminated doctype declaration (missing '>')
    UnterminatedDoctype,
    /// unterminated doctype subset (missing ']')
    UnterminatedDoctypeSubset,
    /// unterminated tag (missing '>')
    UnterminatedTag,
    /// unterminated closing tag (missing '>')
    UnterminatedClosingTag,
    /// invalid tag name
    InvalidTagName,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            Error::AttrInvalidName => "invalid attribute name",
            Error::AttrMissingEq => "'=' character not found in attribute",
            Error::AttrMissingQuote => "quote character not found in attribute",
            Error::AttrMissingEndQuote => "end quote character not found in attribute",
            Error::AttrInvalidQuote => "invalid quote character in attribute",
            Error::UnterminatedEntity => "unterminated entity (missing ';')",
            Error::InvalidNamedEntity => "invalid named entity",
            Error::InvalidNumericEntity => "invalid numeric entity",
            Error::UnterminatedComment => "unterminated comment (missing '-->')",
            Error::UnterminatedPi => "unterminated PI (missing '?>')",
            Error::UnterminatedCdata => "unterminated CDATA section (missing ']]>')",
            Error::UnterminatedDoctype => "unterminated doctype declaration (missing '>')",
            Error::UnterminatedDoctypeSubset => "unterminated doctype subset (missing ']')",
            Error::UnterminatedTag => "unterminated tag (missing '>')",
            Error::UnterminatedClosingTag => "unterminated closing tag (missing '>')",
            Error::InvalidTagName => "invalid tag name",
        };
        f.write_str(msg)
    }
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
            return Some(Err(Error::AttrInvalidName));
        };
        let (start, rest) = self.text.split_at(eq);
        let start = start.trim_matches(WHITESPACE);
        let rest = rest[1..].trim_start_matches(WHITESPACE);
        let mut it = rest.char_indices();
        let Some((_, quote)) = it.next() else {
            self.text = "";
            return Some(Err(Error::AttrMissingQuote));
        };
        if quote != '\'' && quote != '"' {
            self.text = "";
            return Some(Err(Error::AttrInvalidQuote));
        }
        let val_end = loop {
            match it.next() {
                Some((i, c)) if c == quote => break i,
                Some((_, _)) => {}
                None => {
                    self.text = "";
                    return Some(Err(Error::AttrMissingEndQuote));
                }
            }
        };
        self.text = it.as_str();
        if start == "" {
            return Some(Err(Error::AttrInvalidName));
        }
        Some(Ok((start, Text::Escaped(&rest[1..val_end]))))
    }
}

/// A string that can contain XML entity references.
///
/// This type is an iterator of characters.
///
/// To compare equality to a string, use the [`PartialEq<str>`] or
/// [`PartialEq<&str>`] impl.
///
/// To convert to a string, use
/// [`Iterator::collect::<Result<String, txml::Error>>`].
#[derive(Clone, Eq, Debug)]
pub enum Text<'a> {
    /// Text interpreted as-is, without any replacements.
    Verbatim(&'a str),
    /// Text possibly interpreted with XML entity references.
    Escaped(&'a str),
}

impl<'a> Iterator for Text<'a> {
    type Item = Result<char, Error>;

    fn next(&mut self) -> Option<Result<char, Error>> {
        match *self {
            Text::Escaped(ref mut s) if s.starts_with('&') => {
                let Some(semi) = s.find(';') else {
                    *s = "";
                    return Some(Err(Error::UnterminatedEntity));
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
                                return Some(Err(Error::InvalidNumericEntity));
                            }
                        }
                    }
                    _ => {
                        *s = "";
                        return Some(Err(Error::InvalidNamedEntity));
                    }
                }
            }
            Text::Verbatim(ref mut s) | Text::Escaped(ref mut s) => {
                let mut it = s.chars();
                let c = it.next()?;
                *s = it.as_str();
                Some(Ok(c))
            }
        }
    }
}

impl<'a> PartialEq for Text<'a> {
    fn eq(&self, other: &Text<'a>) -> bool {
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
    pub fn new(doc: &'a str) -> Parser<'a> {
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

    // returns err on unclosed quote
    fn consume_to_char_ignoring_quoted_sections(
        &mut self,
        to_chars: &[char],
    ) -> Result<Option<(char, &'a str)>, Error> {
        let mut it = self.doc.char_indices();
        while let Some((i, c)) = it.next() {
            for &to_c in to_chars {
                if c == to_c {
                    let ret = &self.doc[0..i];
                    self.doc = &self.doc[i + to_c.len_utf8()..];
                    return Ok(Some((c, ret)));
                }
            }
            if c == '"' || c == '\'' {
                loop {
                    if let Some((_, inner_c)) = it.next() {
                        if inner_c == c {
                            break;
                        }
                    } else {
                        return Err(Error::AttrMissingEndQuote);
                    }
                }
            }
        }
        Ok(None)
    }

    fn next_inner(&mut self) -> Result<Option<Event<'a>>, Error> {
        let ev = if let Some(tag) = self.self_closing.take() {
            Event::Close(tag)
        } else if self.consume("<?") {
            Event::Pi(self.consume_to("?>").ok_or(Error::UnterminatedPi)?)
        } else if self.consume("<!DOCTYPE") {
            let (c, name) = self
                .consume_to_char_ignoring_quoted_sections(&['[', '>'])?
                .ok_or(Error::UnterminatedDoctype)?;
            if c == '[' {
                let (_, body) = self
                    .consume_to_char_ignoring_quoted_sections(&[']'])?
                    .ok_or(Error::UnterminatedDoctypeSubset)?;
                let body = body.trim_matches(WHITESPACE);
                let _ws = self.consume_to(">").ok_or(Error::UnterminatedDoctype)?;
                Event::Doctype(name.trim_matches(WHITESPACE), body)
            } else {
                Event::Doctype(name.trim_matches(WHITESPACE), "")
            }
        } else if self.consume("<!--") {
            Event::Comment(self.consume_to("-->").ok_or(Error::UnterminatedComment)?)
        } else if self.consume("<![CDATA[") {
            Event::Text(Text::Verbatim(
                self.consume_to("]]>").ok_or(Error::UnterminatedCdata)?,
            ))
        } else if self.consume("</") {
            let tag = self
                .consume_to(">")
                .ok_or(Error::UnterminatedClosingTag)?
                .trim_matches(WHITESPACE);
            if tag == "" {
                return Err(Error::InvalidTagName);
            }
            Event::Close(tag)
        } else if self.consume("<") {
            let (_, content) = self
                .consume_to_char_ignoring_quoted_sections(&['>'])?
                .ok_or(Error::UnterminatedTag)?;
            let (mut tag, rest) = content.split_once(WHITESPACE).unwrap_or((content, ""));
            if tag == "" {
                return Err(Error::InvalidTagName);
            }
            let mut attrs = rest.trim_matches(WHITESPACE);
            if tag.ends_with('/') {
                tag = tag[..tag.len() - 1].trim_end_matches(WHITESPACE);
                self.self_closing = Some(tag);
                if attrs != "" {
                    return Err(Error::InvalidTagName);
                }
            } else if attrs.ends_with('/') {
                self.self_closing = Some(tag);
                attrs = attrs[..attrs.len() - 1].trim_end_matches(WHITESPACE);
            }
            Event::Open(tag, Attrs { text: attrs })
        } else if self.doc.starts_with("&") {
            if let Some(i) = self.doc.find(';') {
                let ret = &self.doc[..=i];
                self.doc = &self.doc[i + 1..];
                Event::Text(Text::Escaped(ret))
            } else {
                let i = self.doc.find('<').unwrap_or_else(|| self.doc.len());
                let ret = &self.doc[..i];
                self.doc = &self.doc[i..];
                Event::Text(Text::Escaped(ret))
            }
        } else if !self.doc.is_empty() {
            let i = self.doc.find(['<', '&']).unwrap_or_else(|| self.doc.len());
            let ret = &self.doc[..i];
            self.doc = &self.doc[i..];
            Event::Text(Text::Verbatim(ret))
        } else {
            return Ok(None);
        };
        Ok(Some(ev))
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<Event<'a>, Error>;

    fn next(&mut self) -> Option<Result<Event<'a>, Error>> {
        match self.next_inner() {
            Ok(Some(ev)) => Some(Ok(ev)),
            Ok(None) => None,
            Err(e) => {
                self.doc = "";
                self.self_closing = None;
                Some(Err(e))
            }
        }
    }
}
