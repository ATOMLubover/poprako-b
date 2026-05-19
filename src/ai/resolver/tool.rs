use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Parameters,
    pub strict: Option<bool>,
}

#[derive(Debug)]
pub struct Parameters {
    pub r#type: String,
    pub properties: HashMap<String, Property>,
    pub required: Option<Vec<String>>,
    pub additional_properties: Option<bool>,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            r#type: "object".to_string(),
            properties: HashMap::new(),
            required: None,
            additional_properties: None,
        }
    }
}

#[derive(Debug)]
pub enum Property {
    String {
        description: String,
        r#enum: Option<Vec<String>>,
    },
    Number {
        description: String,
        r#enum: Option<Vec<f64>>,
    },
    Boolean {
        description: String,
    },
    Array {
        description: String,
    },
    Object {
        description: String,
    },
}

#[derive(Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
