use std::{fmt::Debug, str::FromStr};
use txml::{Event, Parser};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MessageKind {
    Request,
    Event,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Protocol {
    pub name: String,
    pub copyright: String,
    pub description: Option<Description>,
    pub interfaces: Vec<Interface>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Interface {
    pub name: String,
    pub version: u32,
    pub description: Option<Description>,
    pub requests: Vec<Message>,
    pub events: Vec<Message>,
    pub enums: Vec<Enum>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Message {
    pub name: String,
    pub destructor: bool,
    pub since: u32,
    pub deprecated_since: Option<u32>,
    pub description: Option<Description>,
    pub args: Vec<Arg>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Arg {
    pub name: String,
    pub kind: ArgKind,
    pub summary: Option<String>,
    pub interface: Option<String>,
    pub allow_null: bool,
    pub enumeration: Option<String>,
    pub description: Option<Description>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum ArgKind {
    #[default]
    NewId,
    Int,
    Uint,
    Fixed,
    String,
    Object,
    Array,
    Fd,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Enum {
    pub name: String,
    pub since: u32,
    pub bitfield: bool,
    pub description: Option<Description>,
    pub deprecated_since: Option<u32>,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Entry {
    pub name: String,
    pub value: u32,
    pub summary: Option<String>,
    pub since: u32,
    pub deprecated_since: Option<u32>,
    pub description: Option<Description>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Description {
    pub summary: String,
    pub body: String,
}

impl FromStr for ArgKind {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "new_id" => Ok(ArgKind::NewId),
            "int" => Ok(ArgKind::Int),
            "uint" => Ok(ArgKind::Uint),
            "fixed" => Ok(ArgKind::Fixed),
            "string" => Ok(ArgKind::String),
            "object" => Ok(ArgKind::Object),
            "array" => Ok(ArgKind::Array),
            "fd" => Ok(ArgKind::Fd),
            _ => Err(()),
        }
    }
}

pub struct ParseContext<'a> {
    pub parser: txml::Parser<'a>,
    pub attrs: Option<txml::Attrs<'a>>,
}

impl<'a> ParseContext<'a> {
    pub fn next(&mut self) -> Option<Event<'a>> {
        Some(self.parser.next()?)
    }

    pub fn attr<T>(&self, name: &str) -> Option<T>
    where
        T: FromStr,
    {
        self.attrs
            .clone()?
            .filter(|&(k, _)| k == name)
            .map(|(_, v)| v)
            .next()?
            .collect::<String>()
            .parse::<T>()
            .ok()
    }

    pub fn parse(&mut self) -> Option<Protocol> {
        Some(loop {
            match self.next()? {
                Event::Open(name, attrs) if name == "protocol" => {
                    self.attrs = Some(attrs);
                    break self.protocol()?;
                }
                Event::Close(name) if name == "protocol" => return None,
                _ => {}
            }
        })
    }

    pub fn protocol(&mut self) -> Option<Protocol> {
        let mut protocol = Protocol::default();
        protocol.name = self.attr("name")?;
        Some(loop {
            match self.next()? {
                Event::Open(name, attrs) => {
                    self.attrs = Some(attrs);
                    match &*name {
                        "copyright" => protocol.copyright = self.copyright()?,
                        "description" => protocol.description = self.description()?.into(),
                        "interface" => protocol.interfaces.push(self.interface()?),
                        _ => return None,
                    }
                }
                Event::Close(name) if name == "protocol" => break protocol,
                Event::Close(..) => return None,
                Event::Text(..) | Event::Comment(..) | Event::Pi(..) | Event::Doctype(..) => {}
            }
        })
    }

    pub fn copyright(&mut self) -> Option<String> {
        let mut body = String::new();
        Some(loop {
            match self.next()? {
                Event::Text(text) => body.extend(text),
                Event::Close(name) if name == "copyright" => break body,
                Event::Open(..) | Event::Close(..) => return None,
                Event::Comment(..) | Event::Pi(..) | Event::Doctype(..) => {}
            }
        })
    }

    pub fn interface(&mut self) -> Option<Interface> {
        let mut interface = Interface::default();
        interface.name = self.attr("name")?;
        interface.version = self.attr("version")?;
        Some(loop {
            match self.next()? {
                Event::Open(name, attrs) => {
                    self.attrs = Some(attrs);
                    match &*name {
                        "description" => interface.description = self.description()?.into(),
                        "request" => interface.requests.push(self.message()?),
                        "event" => interface.events.push(self.message()?),
                        "enum" => interface.enums.push(self.enumeration()?),
                        _ => return None,
                    }
                }
                Event::Close(name) if name == "interface" => break interface,
                Event::Close(..) => return None,
                Event::Text(..) | Event::Comment(..) | Event::Pi(..) | Event::Doctype(..) => {}
            }
        })
    }

    pub fn message(&mut self) -> Option<Message> {
        let mut message = Message::default();
        message.name = self.attr("name")?;
        message.destructor = self
            .attr("type")
            .map(|t: String| t == "destructor")
            .unwrap_or(false);
        message.since = self.attr("since").unwrap_or(1);
        message.deprecated_since = self.attr("deprecated-since");
        Some(loop {
            match self.next()? {
                Event::Open(name, attrs) => {
                    self.attrs = Some(attrs);
                    match &*name {
                        "description" => message.description = self.description()?.into(),
                        "arg" => message.args.push(self.arg()?),
                        _ => return None,
                    }
                }
                Event::Close(name) if name == "request" || name == "event" => break message,
                Event::Close(..) => return None,
                Event::Text(..) | Event::Comment(..) | Event::Pi(..) | Event::Doctype(..) => {}
            }
        })
    }

    pub fn arg(&mut self) -> Option<Arg> {
        let mut arg = Arg::default();
        arg.name = self.attr("name")?;
        arg.kind = self.attr("type")?;
        arg.summary = self.attr("summary");
        arg.interface = self.attr("interface");
        arg.allow_null = self.attr("allow-null").unwrap_or(false);
        arg.enumeration = self.attr("enum");
        Some(loop {
            match self.next()? {
                Event::Open(name, attrs) if name == "description" => {
                    self.attrs = Some(attrs);
                    arg.description = self.description()?.into();
                }
                Event::Close(name) if name == "arg" => break arg,
                Event::Open(..) | Event::Close(..) => return None,
                Event::Text(..) | Event::Comment(..) | Event::Pi(..) | Event::Doctype(..) => {}
            }
        })
    }

    pub fn enumeration(&mut self) -> Option<Enum> {
        let mut enumeration = Enum::default();
        enumeration.name = self.attr("name")?;
        enumeration.since = self.attr("since").unwrap_or(1);
        enumeration.deprecated_since = self.attr("deprecated-since");
        enumeration.bitfield = self.attr("bitfield").unwrap_or(false);
        Some(loop {
            match self.next()? {
                Event::Open(name, attrs) => {
                    self.attrs = Some(attrs);
                    match &*name {
                        "description" => enumeration.description = self.description()?.into(),
                        "entry" => enumeration.entries.push(self.entry()?),
                        _ => return None,
                    }
                }
                Event::Close(name) if name == "enum" => break enumeration,
                Event::Close(..) => return None,
                Event::Text(..) | Event::Comment(..) | Event::Pi(..) | Event::Doctype(..) => {}
            }
        })
    }

    pub fn entry(&mut self) -> Option<Entry> {
        let mut entry = Entry::default();
        entry.name = self.attr("name")?;
        entry.value = {
            let value: String = self.attr("value")?;
            let (str, radix) = if value.starts_with("0x") {
                (&value[2..], 16)
            } else {
                (&value[..], 10)
            };
            u32::from_str_radix(str, radix).ok()?
        };
        entry.summary = self.attr("summary");
        entry.since = self.attr("since").unwrap_or(1);
        entry.deprecated_since = self.attr("deprecated-since");
        Some(loop {
            match self.next()? {
                Event::Open(name, attrs) if name == "description" => {
                    self.attrs = Some(attrs);
                    entry.description = self.description()?.into();
                }
                Event::Close(name) if name == "entry" => break entry,
                Event::Open(..) | Event::Close(..) => return None,
                Event::Text(..) | Event::Comment(..) | Event::Pi(..) | Event::Doctype(..) => {}
            }
        })
    }

    pub fn description(&mut self) -> Option<Description> {
        let mut description = Description::default();
        description.summary = self.attr("summary")?;
        Some(loop {
            match self.next()? {
                Event::Text(text) => description.body.extend(text),
                Event::Close(name) if name == "description" => break description,
                Event::Open(..) | Event::Close(..) => return None,
                Event::Comment(..) | Event::Pi(..) | Event::Doctype(..) => {}
            }
        })
    }
}

fn main() {
    const XML: &'static str = r#"<?xml version="1.0" encoding="UTF-8"?>
<protocol name="test_protocol">
  <copyright>Test Copyright</copyright>
  <description summary="Test protocol">Protocol description body.</description>
  <interface name="test_interface" version="2">
    <description summary="Test interface">Interface description.</description>
    <request name="test_request" since="2" deprecated-since="3">
      <description summary="Test request">Request description.</description>
      <arg name="id" type="new_id"/>
      <arg name="num" type="int"/>
      <arg name="count" type="uint"/>
      <arg name="fixed_val" type="fixed"/>
      <arg name="text" type="string" allow-null="true"/>
      <arg name="obj" type="object" interface="test_interface"/>
      <arg name="data" type="array"/>
      <arg name="fd" type="fd"/>
      <arg name="enum_arg" type="uint" enum="test_enum" summary="Enum arg"/>
    </request>
    <request name="destroy" type="destructor"/>
    <event name="test_event" deprecated-since="5">
      <arg name="value" type="string"/>
    </event>
    <enum name="test_enum" deprecated-since="4">
      <description summary="Test enum">Enum description.</description>
      <entry name="val_one" value="1" summary="First value"/>
      <entry name="val_hex" value="0xff" since="2" deprecated-since="5">
        <description summary="Hex value">Entry description.</description>
      </entry>
    </enum>
    <enum name="flags" bitfield="true" since="2">
      <entry name="flag_a" value="1"/>
      <entry name="flag_b" value="2" deprecated-since="6"/>
    </enum>
  </interface>
</protocol>"#;

    const RESULT: &'static str = r#"Protocol {
    name: "test_protocol",
    copyright: "Test Copyright",
    description: Some(
        Description {
            summary: "Test protocol",
            body: "Protocol description body.",
        },
    ),
    interfaces: [
        Interface {
            name: "test_interface",
            version: 2,
            description: Some(
                Description {
                    summary: "Test interface",
                    body: "Interface description.",
                },
            ),
            requests: [
                Message {
                    name: "test_request",
                    destructor: false,
                    since: 2,
                    deprecated_since: Some(
                        3,
                    ),
                    description: Some(
                        Description {
                            summary: "Test request",
                            body: "Request description.",
                        },
                    ),
                    args: [
                        Arg {
                            name: "id",
                            kind: NewId,
                            summary: None,
                            interface: None,
                            allow_null: false,
                            enumeration: None,
                            description: None,
                        },
                        Arg {
                            name: "num",
                            kind: Int,
                            summary: None,
                            interface: None,
                            allow_null: false,
                            enumeration: None,
                            description: None,
                        },
                        Arg {
                            name: "count",
                            kind: Uint,
                            summary: None,
                            interface: None,
                            allow_null: false,
                            enumeration: None,
                            description: None,
                        },
                        Arg {
                            name: "fixed_val",
                            kind: Fixed,
                            summary: None,
                            interface: None,
                            allow_null: false,
                            enumeration: None,
                            description: None,
                        },
                        Arg {
                            name: "text",
                            kind: String,
                            summary: None,
                            interface: None,
                            allow_null: true,
                            enumeration: None,
                            description: None,
                        },
                        Arg {
                            name: "obj",
                            kind: Object,
                            summary: None,
                            interface: Some(
                                "test_interface",
                            ),
                            allow_null: false,
                            enumeration: None,
                            description: None,
                        },
                        Arg {
                            name: "data",
                            kind: Array,
                            summary: None,
                            interface: None,
                            allow_null: false,
                            enumeration: None,
                            description: None,
                        },
                        Arg {
                            name: "fd",
                            kind: Fd,
                            summary: None,
                            interface: None,
                            allow_null: false,
                            enumeration: None,
                            description: None,
                        },
                        Arg {
                            name: "enum_arg",
                            kind: Uint,
                            summary: Some(
                                "Enum arg",
                            ),
                            interface: None,
                            allow_null: false,
                            enumeration: Some(
                                "test_enum",
                            ),
                            description: None,
                        },
                    ],
                },
                Message {
                    name: "destroy",
                    destructor: true,
                    since: 1,
                    deprecated_since: None,
                    description: None,
                    args: [],
                },
            ],
            events: [
                Message {
                    name: "test_event",
                    destructor: false,
                    since: 1,
                    deprecated_since: Some(
                        5,
                    ),
                    description: None,
                    args: [
                        Arg {
                            name: "value",
                            kind: String,
                            summary: None,
                            interface: None,
                            allow_null: false,
                            enumeration: None,
                            description: None,
                        },
                    ],
                },
            ],
            enums: [
                Enum {
                    name: "test_enum",
                    since: 1,
                    bitfield: false,
                    description: Some(
                        Description {
                            summary: "Test enum",
                            body: "Enum description.",
                        },
                    ),
                    deprecated_since: Some(
                        4,
                    ),
                    entries: [
                        Entry {
                            name: "val_one",
                            value: 1,
                            summary: Some(
                                "First value",
                            ),
                            since: 1,
                            deprecated_since: None,
                            description: None,
                        },
                        Entry {
                            name: "val_hex",
                            value: 255,
                            summary: None,
                            since: 2,
                            deprecated_since: Some(
                                5,
                            ),
                            description: Some(
                                Description {
                                    summary: "Hex value",
                                    body: "Entry description.",
                                },
                            ),
                        },
                    ],
                },
                Enum {
                    name: "flags",
                    since: 2,
                    bitfield: true,
                    description: None,
                    deprecated_since: None,
                    entries: [
                        Entry {
                            name: "flag_a",
                            value: 1,
                            summary: None,
                            since: 1,
                            deprecated_since: None,
                            description: None,
                        },
                        Entry {
                            name: "flag_b",
                            value: 2,
                            summary: None,
                            since: 1,
                            deprecated_since: Some(
                                6,
                            ),
                            description: None,
                        },
                    ],
                },
            ],
        },
    ],
}"#;

    let mut ctx = ParseContext {
        parser: Parser::new(XML),
        attrs: None,
    };
    let result = ctx.parse().unwrap();
    assert_eq!(format!("{result:#?}"), RESULT);
}
