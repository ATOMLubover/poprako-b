pub struct BotPrompt;

impl BotPrompt {
    pub fn assemble() -> String {
        format!(
            "{}\n\n{}\n\n{}",
            include_str!("prompts/persona.txt"),
            include_str!("prompts/rules.txt"),
            include_str!("prompts/knowledge.txt"),
        )
    }
}
