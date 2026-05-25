pub struct BotPrompt;

impl BotPrompt {
    pub fn assemble() -> anyhow::Result<String> {
        let dir = super::memory_dir().join("prompts");
        let persona = std::fs::read_to_string(dir.join("persona.txt"))?;
        let rules = std::fs::read_to_string(dir.join("rules.txt"))?;
        let knowledge = std::fs::read_to_string(dir.join("knowledge.txt"))?;
        Ok(format!("{}\n\n{}\n\n{}", persona, rules, knowledge))
    }
}
