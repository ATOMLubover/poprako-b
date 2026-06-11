// ---------------------------------------------------------------------------
// System prompt types — hierarchical markdown
// ---------------------------------------------------------------------------
//
// Rendered output:
//
//   # System Prompt Title
//
//   ## Section Title
//
//   section body content
//
//   ## Plugin Group
//
//   ### Plugin SubTitle
//
//   plugin body
//
//   ### Another Plugin
//
//   another body

/// A leaf sub-section rendered as `### Title` — used for individual plugin
/// entries under a `##` plugin group.
#[derive(Debug, Clone)]
pub struct SystemPromptSubSection {
    title: String,
    body: String,
}

impl SystemPromptSubSection {
    pub fn new(title: String, body: String) -> Self {
        Self { title, body }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.title.trim().is_empty() {
            anyhow::bail!("system prompt sub-section title cannot be empty");
        }
        if self.title.contains('\n') {
            anyhow::bail!("system prompt sub-section title must be a single line");
        }
        if self.title.trim_start().starts_with('#') {
            anyhow::bail!("system prompt sub-section title must not start with '#'");
        }
        if self.body.trim().is_empty() {
            anyhow::bail!("system prompt sub-section body cannot be empty");
        }

        Ok(())
    }
}

/// The content of a `##`-level section.
#[derive(Debug, Clone)]
pub enum SectionContent {
    /// A plain body (most file-loaded sections).
    Body(String),
    /// Nested `###` sub-sections (e.g. the plugin group).
    SubSections(Vec<SystemPromptSubSection>),
}

/// A section rendered as `## Title`.
#[derive(Debug, Clone)]
pub struct SystemPromptSection {
    title: String,
    content: SectionContent,
}

impl SystemPromptSection {
    pub fn new(title: String, content: SectionContent) -> Self {
        Self { title, content }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn content(&self) -> &SectionContent {
        &self.content
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.title.trim().is_empty() {
            anyhow::bail!("system prompt section title cannot be empty");
        }
        if self.title.contains('\n') {
            anyhow::bail!("system prompt section title must be a single line");
        }
        if self.title.trim_start().starts_with('#') {
            anyhow::bail!("system prompt section title must not start with '#'");
        }
        match &self.content {
            SectionContent::Body(body) => {
                if body.trim().is_empty() {
                    anyhow::bail!(
                        "system prompt section '{}' body cannot be empty",
                        self.title
                    );
                }
            }
            SectionContent::SubSections(subs) => {
                for sub in subs {
                    sub.validate()?;
                }
            }
        }
        Ok(())
    }
}

/// The root system prompt — an `#` title followed by `Vec<SystemPromptSection>`.
#[derive(Debug, Clone)]
pub struct SystemPrompt {
    title: String,
    sections: Vec<SystemPromptSection>,
}

impl SystemPrompt {
    pub fn new(title: String, sections: Vec<SystemPromptSection>) -> Self {
        Self { title, sections }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn sections(&self) -> &[SystemPromptSection] {
        &self.sections
    }

    /// Consume and return the sections (for merging with plugin sections).
    pub fn into_sections(self) -> Vec<SystemPromptSection> {
        self.sections
    }

    /// Render the system prompt as a markdown string.
    ///
    /// Output:
    /// ```text
    /// # Title
    ///
    /// ## Section
    ///
    /// body
    ///
    /// ## PluginGroup
    ///
    /// ### Sub
    ///
    /// sub body
    /// ```
    /// Ends with exactly one trailing newline.
    pub fn render(&self) -> String {
        let mut buf = String::new();

        // Top-level heading
        buf.push_str("# ");
        buf.push_str(self.title.trim());
        buf.push('\n');

        for section in &self.sections {
            buf.push('\n');
            buf.push_str("## ");
            buf.push_str(section.title.trim());
            buf.push('\n');

            match &section.content {
                SectionContent::Body(body) => {
                    buf.push('\n');
                    buf.push_str(body.trim());
                    buf.push('\n');
                }
                SectionContent::SubSections(subs) => {
                    for sub in subs {
                        buf.push('\n');
                        buf.push_str("### ");
                        buf.push_str(sub.title.trim());
                        buf.push('\n');
                        buf.push('\n');
                        buf.push_str(sub.body.trim());
                        buf.push('\n');
                    }
                }
            }
        }

        // Normalise trailing newline.
        while buf.ends_with("\n\n") {
            buf.pop();
        }
        if !buf.is_empty() && !buf.ends_with('\n') {
            buf.push('\n');
        }

        buf
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(title: &str, body: &str) -> SystemPromptSubSection {
        SystemPromptSubSection::new(title.into(), body.into())
    }

    fn body_section(title: &str, body: &str) -> SystemPromptSection {
        SystemPromptSection::new(title.into(), SectionContent::Body(body.into()))
    }

    fn group_section(title: &str, subs: Vec<SystemPromptSubSection>) -> SystemPromptSection {
        SystemPromptSection::new(title.into(), SectionContent::SubSections(subs))
    }

    // -- SystemPromptSubSection::validate --

    #[test]
    fn validate_sub_rejects_empty_title() {
        assert!(sub("  ", "body").validate().is_err());
    }

    #[test]
    fn validate_sub_rejects_empty_body() {
        assert!(sub("title", "  ").validate().is_err());
    }

    #[test]
    fn validate_sub_rejects_hash_prefix() {
        let err = sub("# Bad", "body").validate().unwrap_err().to_string();
        assert!(err.contains("'#'"));
    }

    #[test]
    fn validate_sub_rejects_multiline_title() {
        assert!(sub("A\nB", "body").validate().is_err());
    }

    #[test]
    fn validate_sub_accepts_valid() {
        assert!(sub("Valid", "Content.").validate().is_ok());
    }

    // -- SystemPromptSection::validate --

    #[test]
    fn validate_section_rejects_empty_title() {
        let s = body_section("  ", "body");
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_section_rejects_empty_body() {
        let s = body_section("Title", "  ");
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_section_propagates_sub_error() {
        let s = group_section("Group", vec![sub("ok", "body"), sub("", "empty")]);
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_section_accepts_empty_subs() {
        // A group with no sub-sections is valid (renders just the ## heading).
        assert!(group_section("Group", vec![]).validate().is_ok());
    }

    #[test]
    fn validate_section_accepts_valid_body() {
        assert!(body_section("Title", "Some body.").validate().is_ok());
    }

    // -- SystemPrompt::render --

    #[test]
    fn empty_sections_renders_title_only() {
        let prompt = SystemPrompt::new("白杨子指导".into(), vec![]);
        let r = prompt.render();
        assert_eq!(r, "# 白杨子指导\n");
    }

    #[test]
    fn body_sections_render_as_h2() {
        let prompt = SystemPrompt::new(
            "白杨子指导".into(),
            vec![
                body_section("人格", "You are a helpful bot."),
                body_section("场景", "You are in a chat room."),
            ],
        );
        let r = prompt.render();

        assert!(r.starts_with("# 白杨子指导\n"));
        let renge_pos = r.find("## 人格").unwrap();
        let scene_pos = r.find("## 场景").unwrap();
        assert!(renge_pos < scene_pos);

        assert!(r.contains("You are a helpful bot."));
        assert!(r.contains("You are in a chat room."));
    }

    #[test]
    fn group_section_renders_h3_subs() {
        let prompt = SystemPrompt::new(
            "白杨子指导".into(),
            vec![
                body_section("人格", "Bot persona."),
                group_section(
                    "插件说明",
                    vec![
                        sub("灵光一闪", "Inspiration plugin."),
                        sub("记忆碎片", "Memory shard plugin."),
                    ],
                ),
            ],
        );
        let r = prompt.render();

        // Order: # title, ## 人格, ## 插件说明, ### 灵光一闪, ### 记忆碎片
        let h1 = r.find("# 白杨子指导").unwrap();
        let h2_renge = r.find("## 人格").unwrap();
        let h2_plugin = r.find("## 插件说明").unwrap();
        let h3_ling = r.find("### 灵光一闪").unwrap();
        let h3_jiyi = r.find("### 记忆碎片").unwrap();

        assert!(h1 < h2_renge);
        assert!(h2_renge < h2_plugin);
        assert!(h2_plugin < h3_ling);
        assert!(h3_ling < h3_jiyi);

        assert!(r.contains("Inspiration plugin."));
        assert!(r.contains("Memory shard plugin."));
    }

    #[test]
    fn trailing_newline_normalized() {
        let prompt = SystemPrompt::new("T".into(), vec![body_section("S", "b")]);
        let r = prompt.render();
        assert!(r.ends_with('\n'));
        assert!(!r.ends_with("\n\n"));
    }
}
