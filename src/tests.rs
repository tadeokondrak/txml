extern crate alloc;
use crate::{Attrs, Error, Event, Parser, Text};
use alloc::{string::String, vec::Vec};

macro_rules! extract {
    ($ev:expr, $pat:pat) => {
        let $pat = $ev else {
            panic!("unexpected event type")
        };
    };
}

#[track_caller]
fn only_event(s: &str) -> Result<Event<'_>, Error> {
    let mut p = Parser::new(s);
    let ev = p.next().unwrap();
    assert_eq!(p.next(), None, "extra event found");
    ev
}

#[track_caller]
fn only_text(s: &str) -> Result<String, Error> {
    let mut res = String::new();
    for ev in Parser::new(s) {
        extract!(ev, Ok(Event::Text(text)));
        for c in text {
            let c = c?;
            res.push(c);
        }
    }
    Ok(res)
}

fn all_events(text: &str) -> Result<Vec<Event<'_>>, Error> {
    Parser::new(text).collect::<Result<Vec<_>, _>>()
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
    assert_eq!(only_text(DOC), Ok("<>&'\"".into()));
}

#[test]
fn numeric_entities() {
    const DOC: &'static str = "&#60;&#x3E;";
    let events = all_events(DOC).unwrap();
    extract!(events[0].clone(), Event::Text(text_0));
    extract!(events[1].clone(), Event::Text(text_1));
    assert_eq!(events.len(), 2);
    assert_eq!(text_0, Text::Escaped("&#60;"));
    assert_eq!(text_0.collect::<Result<String, Error>>(), Ok("<".into()));
    assert_eq!(text_1, Text::Escaped("&#x3E;"));
    assert_eq!(text_1.collect::<Result<String, Error>>(), Ok(">".into()));
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
fn invalid_hex_numeric_entity_size() {
    const DOC: &'static str = "&#x1000000000;";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(text, Text::Escaped(DOC));
    assert_eq!(
        text.collect::<Result<String, Error>>(),
        Err(Error::INVALID_NUMERIC_ENTITY)
    );
}

#[test]
fn invalid_hex_numeric_entity_chars() {
    const DOC: &'static str = "&#xGHIJ;";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(
        text.collect::<Result<String, Error>>(),
        Err(Error::INVALID_NUMERIC_ENTITY)
    );
}

#[test]
fn system_doctype() {
    const DOC: &'static str = r#"<?xml version="1.0"?>
<!DOCTYPE greeting SYSTEM "hello.dtd">
<greeting>Hello, world!</greeting>"#;
    assert_eq!(
        all_events(DOC).unwrap(),
        [
            Event::Pi("xml version=\"1.0\""),
            Event::Text(Text::Escaped("\n")),
            Event::Doctype("greeting SYSTEM \"hello.dtd\"", ""),
            Event::Text(Text::Escaped("\n")),
            Event::Open("greeting", Attrs { text: "" }),
            Event::Text(Text::Escaped("Hello, world!")),
            Event::Close("greeting")
        ]
    );
}

#[test]
fn local_doctype() {
    const DOC: &'static str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE greeting [
 <!ELEMENT greeting (#PCDATA)>
]>
<greeting>Hello, world!</greeting>"#;
    assert_eq!(
        all_events(DOC).unwrap(),
        [
            Event::Pi(r#"xml version="1.0" encoding="UTF-8""#),
            Event::Text(Text::Escaped("\n")),
            Event::Doctype("greeting", "<!ELEMENT greeting (#PCDATA)>"),
            Event::Text(Text::Escaped("\n")),
            Event::Open("greeting", Attrs { text: "" }),
            Event::Text(Text::Escaped("Hello, world!")),
            Event::Close("greeting"),
        ]
    );
}

#[test]
fn unterminated_cdata() {
    const DOC: &'static str = "<![CDATA[unclosed";
    let result = only_event(DOC);
    assert_eq!(result, Err(Error::UNTERMINATED_CDATA));
}

#[test]
fn valid_cdata() {
    const DOC: &'static str = "<![CDATA[content]]>";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(text, Text::Verbatim("content"));
}

#[test]
fn empty_cdata() {
    const DOC: &'static str = "<![CDATA[]]>";
    extract!(only_event(DOC), Ok(Event::Text(text)));
    assert_eq!(text, Text::Verbatim(""));
}

#[test]
fn unterminated_attribute_quote() {
    const DOC: &'static str = r#"<element attr="unterminated>"#;
    extract!(only_event(DOC), Ok(Event::Open(_, mut attrs)));
    assert_eq!(attrs.next(), Some(Err(Error::ATTR_MISSING_END_QUOTE)));
}

#[test]
fn self_closing() {
    const DOC: &'static str = "<element attr='value' />";
    let events = all_events(DOC).unwrap();
    assert_eq!(
        events.clone(),
        [
            Event::Open(
                "element",
                Attrs {
                    text: "attr='value'"
                }
            ),
            Event::Close("element")
        ]
    );
    extract!(events[0].clone(), Event::Open(_, mut attrs));
    assert_eq!(attrs.next(), Some(Ok(("attr", Text::Escaped("value")))));
    assert_eq!(attrs.next(), None);
}
