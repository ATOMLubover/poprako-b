#[derive(Debug, Clone, Default)]
pub struct InspiredAnnotation {
    knowledge_id: Option<String>,
}

impl InspiredAnnotation {
    pub fn with_knowledge_id(id: impl Into<String>) -> Self {
        Self {
            knowledge_id: Some(id.into()),
        }
    }

    pub fn knowledge_id(&self) -> Option<&str> {
        self.knowledge_id.as_deref()
    }
}

pub trait IInspirationAnnotated {
    /// Returns the inspiration annotation, which may indicate whether this message is an inspiration injection and which knowledge entry it corresponds to.
    fn inspired_annotation(&self) -> &InspiredAnnotation;

    /// Returns a mutable reference to the inspiration annotation, allowing modification of the inspiration state.
    fn inspired_annotation_mut(&mut self) -> &mut InspiredAnnotation;
}
