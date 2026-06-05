use crate::bot::agent::plugin::inspiration::InspirationAnnotation;
use crate::bot::agent::plugin::inspiration::InspirationState;
use crate::bot::agent::plugin::inspiration::WithInspirationAnnotation;
use crate::bot::agent::plugin::inspiration::WithInspirationState;

#[derive(Default)]
pub struct BotAgentState {
    inspiration_state: InspirationState,
}

impl WithInspirationState for BotAgentState {
    fn inspiration_state_mut(&mut self) -> &mut InspirationState {
        &mut self.inspiration_state
    }
}

#[derive(Default)]
pub struct BotMessageAnnotation {
    inspiration_annotation: InspirationAnnotation,
}

impl WithInspirationAnnotation for BotMessageAnnotation {
    fn inspiration_annotation(&self) -> &InspirationAnnotation {
        &self.inspiration_annotation
    }

    fn inspiration_annotation_mut(&mut self) -> &mut InspirationAnnotation {
        &mut self.inspiration_annotation
    }
}
