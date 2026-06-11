use serde::Serialize;

use crate::ai::resolver::action::Action;
use crate::ai::resolver::tool::IToolCall;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemItem {
    pub id: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginSystemItem {
    pub id: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemMessage {
    embedded: Vec<SystemItem>,
    plugins: Vec<PluginSystemItem>,
}

// ---------------------------------------------------------------------------
// XML serialization (quick-xml, zero-copy)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename = "system")]
struct SystemXml<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    embedded: Option<SectionList<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugins: Option<SectionList<'a>>,
}

#[derive(Serialize)]
struct SectionList<'a> {
    #[serde(rename = "section", default)]
    section: Vec<SectionXml<'a>>,
}

#[derive(Serialize)]
struct SectionXml<'a> {
    #[serde(rename = "@id")]
    id: &'a str,
    #[serde(rename = "@title")]
    title: &'a str,
    #[serde(rename = "$text")]
    content: &'a str,
}

impl SystemMessage {
    pub fn new(embedded: Vec<SystemItem>, plugins: Vec<PluginSystemItem>) -> Self {
        Self { embedded, plugins }
    }

    pub fn render(&self) -> String {
        let xml = SystemXml {
            embedded: collect_sections(&self.embedded),
            plugins: collect_sections(&self.plugins),
        };
        let mut buf = String::new();
        let mut ser = quick_xml::se::Serializer::new(&mut buf);
        ser.indent(' ', 2);
        xml.serialize(ser)
            .expect("XML serialization of SystemMessage should not fail");
        buf
    }
}

/// Map `[T]` → `Option<SectionList>`, returning `None` when empty.
/// Both `SystemItem` and `PluginSystemItem` have the same shape, so a single
/// free function covers both — no copy-paste.
fn collect_sections<'a>(items: &'a [impl AsXmlSection]) -> Option<SectionList<'a>> {
    if items.is_empty() {
        return None;
    }
    Some(SectionList {
        section: items.iter().map(|item| item.as_xml_section()).collect(),
    })
}

trait AsXmlSection {
    fn as_xml_section(&self) -> SectionXml<'_>;
}

impl AsXmlSection for SystemItem {
    fn as_xml_section(&self) -> SectionXml<'_> {
        SectionXml {
            id: &self.id,
            title: &self.title,
            content: self.content.trim_end(),
        }
    }
}

impl AsXmlSection for PluginSystemItem {
    fn as_xml_section(&self) -> SectionXml<'_> {
        SectionXml {
            id: &self.id,
            title: &self.title,
            content: self.content.trim_end(),
        }
    }
}

impl<C> From<SystemMessage> for MessageOwned<C>
where
    C: IToolCall,
{
    fn from(system: SystemMessage) -> Self {
        MessageOwned::System {
            content: system.render(),
        }
    }
}

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum MessageRef<'a, C>
where
    C: IToolCall,
{
    System { content: &'a str },
    User { content: &'a str },
    Assist {
        content: Option<&'a str>,
        tool_calls: Option<&'a [C]>,
        refusal: Option<&'a str>,
    },
    Tool {
        tool_call_id: &'a str,
        content: &'a str,
    },
}

pub enum MessageOwned<C>
where
    C: IToolCall,
{
    System { content: String },
    User { content: String },
    Assist {
        content: Option<String>,
        tool_calls: Option<Vec<C>>,
        refusal: Option<String>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assist,
    Tool,
}

pub trait IMessage:
    From<Action<Self::ToolCall>>
    + for<'a> From<MessageRef<'a, Self::ToolCall>>
    + From<MessageOwned<Self::ToolCall>>
{
    type ToolCall: IToolCall + std::fmt::Debug;

    fn message_ref(&self) -> MessageRef<'_, Self::ToolCall>;

    fn role(&self) -> MessageRole {
        match self.message_ref() {
            MessageRef::System { .. } => MessageRole::System,
            MessageRef::User { .. } => MessageRole::User,
            MessageRef::Assist { .. } => MessageRole::Assist,
            MessageRef::Tool { .. } => MessageRole::Tool,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(id: &str, title: &str, content: &str) -> SystemItem {
        SystemItem { id: id.into(), title: title.into(), content: content.into() }
    }

    fn plug(id: &str, title: &str, content: &str) -> PluginSystemItem {
        PluginSystemItem { id: id.into(), title: title.into(), content: content.into() }
    }

    #[test]
    fn empty_system() {
        let r = SystemMessage::new(vec![], vec![]).render();
        assert!(r.contains("<system"));
        assert!(!r.contains("<embedded>"));
        assert!(!r.contains("<plugins>"));
    }

    #[test]
    fn embedded_before_plugins() {
        let sm = SystemMessage::new(
            vec![sys("a", "A", "e")],
            vec![plug("p", "P", "p")],
        );
        let r = sm.render();
        assert!(r.find("<embedded>").unwrap() < r.find("<plugins>").unwrap());
    }

    #[test]
    fn section_attributes_and_content() {
        let r = SystemMessage::new(
            vec![sys("persona", "Persona", "You are a bot.")],
            vec![],
        ).render();
        assert!(r.contains(r#"<section id="persona" title="Persona">"#));
        assert!(r.contains("You are a bot."));
        assert!(r.contains("<embedded>"));
        assert!(!r.contains("<plugins>"));
    }

    #[test]
    fn plugins_shown_when_non_empty() {
        let r = SystemMessage::new(vec![], vec![plug("p", "P", "hi")]).render();
        assert!(!r.contains("<embedded>"));
        assert!(r.contains("<plugins>"));
    }

    #[test]
    fn xml_escape() {
        let r = SystemMessage::new(
            vec![sys("a", "A", "x < 3 && y > 5")],
            vec![],
        ).render();
        assert!(r.contains("&lt;"));
        assert!(r.contains("&amp;"));
    }

    #[test]
    fn section_order_stable() {
        let r = SystemMessage::new(
            vec![sys("1", "A", ""), sys("2", "B", ""), sys("3", "C", "")],
            vec![],
        ).render();
        let p1 = r.find(r#"id="1""#).unwrap();
        let p2 = r.find(r#"id="2""#).unwrap();
        let p3 = r.find(r#"id="3""#).unwrap();
        assert!(p1 < p2 && p2 < p3);
    }
}
