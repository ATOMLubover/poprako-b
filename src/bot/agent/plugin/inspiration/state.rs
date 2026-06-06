use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct InspiredState {
    pub active_knowledge_ids: HashSet<String>,
}

pub trait IInspirationEmbedded {
    /// Returns a mutable reference to the inspired state, allowing the agent to track which knowledge entries have been injected as inspirations in the current conversation context.
    fn inspired_state_mut(&mut self) -> &mut InspiredState;
}
