use tokio::sync::mpsc;

use crate::bot::agent::plugin::inspiration::{IInspirationAnnotated, IInspirationEmbedded, InspiredAnnotation, InspiredState};
use crate::bot::agent::plugin::review::{IReviewAnnotated, IReviewEmbedded, ReviewAnnotation, ReviewRuntime, SolveKind};
use crate::bot::event::ReviewFollowupEvent;

#[derive(Default)]
pub struct BotAgentState {
    inspiration_state: InspiredState,
    review_runtime: ReviewRuntime,
}

impl IInspirationEmbedded for BotAgentState {
    fn inspired_state_mut(&mut self) -> &mut InspiredState {
        &mut self.inspiration_state
    }
}

impl IReviewEmbedded for BotAgentState {
    fn review_runtime(&self) -> &ReviewRuntime {
        &self.review_runtime
    }

    fn review_runtime_mut(&mut self) -> &mut ReviewRuntime {
        &mut self.review_runtime
    }
}

impl BotAgentState {
    pub fn set_review_event_send(&mut self, send: mpsc::Sender<ReviewFollowupEvent>) {
        self.review_runtime.set_event_send(send);
    }

    pub fn begin_solve(&mut self, kind: SolveKind, respond_id: String) {
        self.review_runtime.begin_solve(kind, respond_id);
    }
}

#[derive(Clone, Default)]
pub struct BotMessageAnnotation {
    inspiration_annotation: InspiredAnnotation,
    review_annotation: ReviewAnnotation,
}

impl IInspirationAnnotated for BotMessageAnnotation {
    fn inspired_annotation(&self) -> &InspiredAnnotation {
        &self.inspiration_annotation
    }

    fn inspired_annotation_mut(&mut self) -> &mut InspiredAnnotation {
        &mut self.inspiration_annotation
    }
}

impl IReviewAnnotated for BotMessageAnnotation {
    fn review_annotation(&self) -> &ReviewAnnotation {
        &self.review_annotation
    }

    fn review_annotation_mut(&mut self) -> &mut ReviewAnnotation {
        &mut self.review_annotation
    }
}
