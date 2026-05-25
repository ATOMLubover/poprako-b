use crate::bot::agent::memory_dir;

pub struct BotPrompt;

impl BotPrompt {
    pub fn system_prompt() -> anyhow::Result<String> {
        let dir = memory_dir().join("prompts");

        let persona = std::fs::read_to_string(dir.join("persona.txt"))?;
        let rules = std::fs::read_to_string(dir.join("rules.txt"))?;

        Ok(format!("{}\n\n{}", persona, rules))
    }
}
