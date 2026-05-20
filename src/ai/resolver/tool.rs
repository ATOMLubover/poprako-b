use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Parameters,
    pub strict: Option<bool>,
}

impl Tool {
    pub fn new(name: &str, description: &str, parameters: Parameters) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
            strict: None,
        }
    }

    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = Some(strict);
        self
    }
}

#[derive(Debug)]
pub struct Parameters {
    pub r#type: String,
    pub properties: HashMap<String, Property>,
    pub required: Option<Vec<String>>,
    pub additional_properties: Option<bool>,
}

impl Parameters {
    pub fn new(r#type: &str) -> Self {
        Self {
            r#type: r#type.to_string(),
            properties: HashMap::new(),
            required: None,
            additional_properties: None,
        }
    }

    /// Add multiple named properties at once.
    ///
    /// Each entry is `(name, property)`. Use [`with_required`](Self::with_required)
    /// to mark specific properties as required.
    pub fn with_properties(mut self, properties: Vec<(&str, Property)>) -> Self {
        properties.into_iter().for_each(|(name, prop)| {
            self.properties.insert(name.to_string(), prop);
        });
        self
    }

    pub fn with_required(mut self, required: Vec<String>) -> Self {
        self.required = Some(required);
        self
    }

    /// Convert to a `serde_json::Value` matching the OpenAI JSON Schema format.
    pub fn to_value(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "type".to_string(),
            serde_json::Value::String(self.r#type.clone()),
        );

        let mut props = serde_json::Map::new();
        for (name, prop) in &self.properties {
            let mut prop_map = serde_json::Map::new();
            prop_map.insert(
                "description".to_string(),
                serde_json::Value::String(prop.description().to_string()),
            );
            prop_map.insert(
                "type".to_string(),
                serde_json::Value::String(prop.typ().to_string()),
            );
            if let Some(enums) = prop.enum_values() {
                prop_map.insert("enum".to_string(), enums);
            }
            props.insert(name.clone(), serde_json::Value::Object(prop_map));
        }
        map.insert("properties".to_string(), serde_json::Value::Object(props));

        if let Some(required) = &self.required {
            let arr: Vec<serde_json::Value> = required
                .iter()
                .map(|r| serde_json::Value::String(r.clone()))
                .collect();
            map.insert("required".to_string(), serde_json::Value::Array(arr));
        }

        if let Some(additional) = self.additional_properties {
            map.insert(
                "additionalProperties".to_string(),
                serde_json::Value::Bool(additional),
            );
        }

        serde_json::Value::Object(map)
    }
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

impl Property {
    fn description(&self) -> &str {
        match self {
            Property::String { description, .. }
            | Property::Number { description, .. }
            | Property::Boolean { description }
            | Property::Array { description }
            | Property::Object { description } => description,
        }
    }

    fn typ(&self) -> &str {
        match self {
            Property::String { .. } => "string",
            Property::Number { .. } => "number",
            Property::Boolean { .. } => "boolean",
            Property::Array { .. } => "array",
            Property::Object { .. } => "object",
        }
    }

    fn enum_values(&self) -> Option<serde_json::Value> {
        match self {
            Property::String { r#enum, .. } => r#enum.as_ref().map(|v| {
                serde_json::Value::Array(
                    v.iter()
                        .map(|e| serde_json::Value::String(e.clone()))
                        .collect(),
                )
            }),
            Property::Number { r#enum, .. } => r#enum.as_ref().map(|v| {
                serde_json::Value::Array(
                    v.iter()
                        .map(|e| {
                            serde_json::Value::Number(
                                serde_json::Number::from_f64(*e)
                                    .expect("NaN/Inf not supported in JSON"),
                            )
                        })
                        .collect(),
                )
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

pub trait IToolCall {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn args(&self) -> &str;
}

impl IToolCall for ToolCall {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn args(&self) -> &str {
        &self.arguments
    }
}
