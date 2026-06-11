#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewMessageSource {
    NormalUser,
    NormalAssistant,
    ReviewFeedbackUser,
    ReviewFollowupAssistant,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewAnnotation {
    respond_id: Option<String>,
    source: Option<ReviewMessageSource>,
}

impl ReviewAnnotation {
    pub fn new(respond_id: String, source: ReviewMessageSource) -> Self {
        Self {
            respond_id: Some(respond_id),
            source: Some(source),
        }
    }

    pub fn respond_id(&self) -> Option<&str> {
        self.respond_id.as_deref()
    }

    pub fn source(&self) -> Option<ReviewMessageSource> {
        self.source
    }
}

pub trait IReviewAnnotated {
    fn review_annotation(&self) -> &ReviewAnnotation;

    fn review_annotation_mut(&mut self) -> &mut ReviewAnnotation;
}
