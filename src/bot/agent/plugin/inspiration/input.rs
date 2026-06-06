fn metadata_value<'a>(meta: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{}: ", key);
    let start = meta.find(&prefix)? + prefix.len();
    let rest = &meta[start..];
    let end = rest.find(',').unwrap_or(rest.len());
    let value = rest[..end].trim();

    if value == "-" || value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub struct MatchInput<'a> {
    sender_nickname: Option<&'a str>,
    sender_channel_nickname: Option<&'a str>,
    body: &'a str,
}

impl<'a> MatchInput<'a> {
    pub fn parse(prompt_text: &'a str) -> Self {
        let (meta, body) = prompt_text
            .split_once('\n')
            .unwrap_or((prompt_text, prompt_text));

        Self {
            sender_nickname: metadata_value(meta, "sender_nickname"),
            sender_channel_nickname: metadata_value(meta, "sender_channel_nickname"),
            body,
        }
    }

    pub fn contains(&self, keyword: &str) -> bool {
        self.body.contains(keyword)
            || self.sender_nickname == Some(keyword)
            || self.sender_channel_nickname == Some(keyword)
    }
}
