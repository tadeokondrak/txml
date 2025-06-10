extern crate alloc;
use crate::{Error, Event, Parser, Text};
use alloc::string::String;

macro_rules! extract {
    ($ev:expr, $pat:pat) => {
        let $pat = $ev else {
            panic!("unexpected event type")
        };
    };
}

fn only_event(s: &str) -> Result<Event<'_>, Error> {
    let mut p = Parser::new(s);
    let ev = p.next().unwrap();
    assert_eq!(p.next(), None, "extra event found");
    ev
}

#[test]
fn no_equals_character_in_attribute() {
    const DOC: &'static str = "<element attr>";
    extract!(only_event(DOC), Ok(Event::Open(_, mut attrs)));
    assert_eq!(attrs.next(), Some(Err(Error::ATTR_MISSING_EQ)));
    assert_eq!(attrs.next(), None);
}

#[test]
fn no_quote_character_in_attribute() {
    const DOC: &'static str = "<element attr=>";
    extract!(only_event(DOC), Ok(Event::Open(_, mut attrs)));
    assert_eq!(attrs.next(), Some(Err(Error::ATTR_MISSING_QUOTE)));
    assert_eq!(attrs.next(), None);
}

#[test]
fn invalid_quote_character_in_attribute() {
    const DOC: &'static str = "<element attr=unquoted>";
    extract!(only_event(DOC), Ok(Event::Open(_, mut attrs)));
    assert_eq!(attrs.next(), Some(Err(Error::ATTR_INVALID_QUOTE)));
    assert_eq!(attrs.next(), None);
}

#[test]
fn extra_whitespace_in_tag_after_attribute() {
    const DOC: &'static str = "<element attr='test' >";
    extract!(only_event(DOC), Ok(Event::Open(_, mut attrs)));
    assert_eq!(attrs.next(), Some(Ok(("attr", Text::Escaped("test")))));
    assert_eq!(attrs.next(), None);
}

#[test]
fn extra_whitespace_in_tag_between_attributes() {
    const DOC: &'static str = "<element attr='test'  attr='test'>";
    extract!(only_event(DOC), Ok(Event::Open(_, mut attrs)));
    assert_eq!(attrs.next(), Some(Ok(("attr", Text::Escaped("test")))));
    assert_eq!(attrs.next(), Some(Ok(("attr", Text::Escaped("test")))));
    assert_eq!(attrs.next(), None);
}

#[test]
fn named_entities() {
    const DOC: &'static str = "&lt;&gt;&amp;&apos;&quot;";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(text, Text::Escaped(DOC));
    assert_eq!(text.collect::<Result<String, Error>>(), Ok("<>&'\"".into()));
}

#[test]
fn numeric_entities() {
    const DOC: &'static str = "&#60;&#x3E;";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(text, Text::Escaped(DOC));
    assert_eq!(text.collect::<Result<String, Error>>(), Ok("<>".into()));
}

#[test]
fn unterminated_named_entity() {
    const DOC: &'static str = "&lt";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(text, Text::Escaped(DOC));
    assert_eq!(
        text.collect::<Result<String, Error>>(),
        Err(Error::UNTERMINATED_ENTITY)
    );
}

#[test]
fn invalid_decimal_numeric_entity() {
    const DOC: &'static str = "&#1000000000;";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(text, Text::Escaped(DOC));
    assert_eq!(
        text.collect::<Result<String, Error>>(),
        Err(Error::INVALID_NUMERIC_ENTITY)
    );
}

#[test]
fn invalid_hex_numeric_entity() {
    const DOC: &'static str = "&#x1000000000;";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(text, Text::Escaped(DOC));
    assert_eq!(
        text.collect::<Result<String, Error>>(),
        Err(Error::INVALID_NUMERIC_ENTITY)
    );
}
