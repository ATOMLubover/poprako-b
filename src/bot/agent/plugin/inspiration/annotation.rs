#[derive(Debug, Clone, Default)]
pub struct InspirationAnnotation {
    inspiration_id: Option<String>,
}

impl InspirationAnnotation {
    pub fn inspiration(id: impl Into<String>) -> Self {
        Self {
            inspiration_id: Some(id.into()),
        }
    }

    pub fn inspiration_id(&self) -> Option<&str> {
        self.inspiration_id.as_deref()
    }
}

pub trait IWithInspirationAnnotation {
    fn inspiration_annotation(&self) -> &InspirationAnnotation;

    fn inspiration_annotation_mut(&mut self) -> &mut InspirationAnnotation;
}

impl IWithInspirationAnnotation for InspirationAnnotation {
    fn inspiration_annotation(&self) -> &InspirationAnnotation {
        self
    }

    fn inspiration_annotation_mut(&mut self) -> &mut InspirationAnnotation {
        self
    }
}
