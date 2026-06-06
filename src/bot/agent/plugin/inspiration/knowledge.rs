use crate::bot::agent::plugin::inspiration::input::MatchInput;
use crate::bot::agent::plugin::inspiration::state::InspirationState;

#[derive(Debug, Clone, Copy)]
pub struct KnowledgeEntry {
    pub id: &'static str,
    pub keywords: &'static [&'static str],
    pub content: &'static str,
}

impl KnowledgeEntry {
    fn matches(&self, input: &MatchInput<'_>) -> bool {
        self.keywords.iter().any(|keyword| input.contains(keyword))
    }
}

fn knowledge_entries() -> &'static [KnowledgeEntry] {
    &[
        KnowledgeEntry {
            id: "member.lb",
            keywords: &["LB"],
            content: "LB：核心开发，负责 poprako 全系列工具的开发",
        },
        KnowledgeEntry {
            id: "member.niuniu",
            keywords: &["牛牛", "灰暗天穹"],
            content: "牛牛 / 灰暗天穹：喜欢剧情、画工、萝莉；巨乳是减分项",
        },
        KnowledgeEntry {
            id: "member.nabai",
            keywords: &["那白"],
            content: "那白：翻译，热爱学习译法，喜欢用告白台词开玩笑",
        },
    ]
}

#[derive(Default)]
pub struct InspirationKnowledge;

impl InspirationKnowledge {
    pub fn match_entries(
        &self,
        input: &MatchInput<'_>,
        state: &InspirationState,
    ) -> Vec<KnowledgeEntry> {
        knowledge_entries()
            .iter()
            .copied()
            .filter(|entry| !state.active_inspiration_ids.contains(entry.id))
            .filter(|entry| entry.matches(input))
            .collect()
    }
}
