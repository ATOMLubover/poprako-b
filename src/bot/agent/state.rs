use crate::bot::agent::plugin::inspiration::IInspirationAnnotated;
use crate::bot::agent::plugin::inspiration::IInspirationEmbedded;
use crate::bot::agent::plugin::inspiration::InspiredAnnotation;
use crate::bot::agent::plugin::inspiration::InspiredState;

#[derive(Default)]
pub struct BotAgentState {
    inspiration_state: InspiredState,
}

impl IInspirationEmbedded for BotAgentState {
    fn inspired_state_mut(&mut self) -> &mut InspiredState {
        &mut self.inspiration_state
    }
}

#[derive(Default)]
pub struct BotMessageAnnotation {
    inspiration_annotation: InspiredAnnotation,
}

impl IInspirationAnnotated for BotMessageAnnotation {
    fn inspired_annotation(&self) -> &InspiredAnnotation {
        &self.inspiration_annotation
    }

    fn inspired_annotation_mut(&mut self) -> &mut InspiredAnnotation {
        &mut self.inspiration_annotation
    }
}
