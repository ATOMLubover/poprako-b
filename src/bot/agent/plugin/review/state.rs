use tokio::sync::mpsc;

use crate::bot::event::ReviewFollowupEvent;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SolveKind {
    #[default]
    Normal,
    ReviewFollowup,
}

#[derive(Default)]
pub struct ReviewRuntime {
    solve_kind: SolveKind,
    respond_id: Option<String>,
    event_send: Option<mpsc::Sender<ReviewFollowupEvent>>,
}

impl ReviewRuntime {
    pub fn begin_solve(&mut self, kind: SolveKind, respond_id: String) {
        self.solve_kind = kind;
        self.respond_id = Some(respond_id);
    }

    pub fn clear_solve(&mut self) {
        self.solve_kind = SolveKind::Normal;
        self.respond_id = None;
    }

    pub fn solve_kind(&self) -> SolveKind {
        self.solve_kind
    }

    pub fn respond_id(&self) -> Option<&str> {
        self.respond_id.as_deref()
    }

    pub fn event_send(&self) -> Option<mpsc::Sender<ReviewFollowupEvent>> {
        self.event_send.clone()
    }

    pub fn set_event_send(&mut self, send: mpsc::Sender<ReviewFollowupEvent>) {
        self.event_send = Some(send);
    }
}

pub trait IReviewEmbedded {
    fn review_runtime(&self) -> &ReviewRuntime;

    fn review_runtime_mut(&mut self) -> &mut ReviewRuntime;
}
