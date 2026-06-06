use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct InspirationState {
    pub active_inspiration_ids: HashSet<String>,
}

pub trait IWithInspirationState {
    fn inspiration_state_mut(&mut self) -> &mut InspirationState;
}

impl IWithInspirationState for InspirationState {
    fn inspiration_state_mut(&mut self) -> &mut InspirationState {
        self
    }
}
