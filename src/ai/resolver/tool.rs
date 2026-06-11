use std::collections::HashMap;

use serde::Deserialize;
use serde::de::Error as DeError;

#[derive(Deserialize)]
struct PropSchema {
    #[serde(rename = "type")]
    typ: String,
    #[serde(default, rename = "description")]
    desc: String,
    #[serde(default, rename = "enum")]
    enum_values: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone)]
pub enum PropDef {
    String {
        desc: String,
        r#enum: Option<Vec<String>>,
    },
    Number {
        desc: String,
        r#enum: Option<Vec<f64>>,
    },
    Boolean {
        desc: String,
    },
    Array {
        desc: String,
    },
    Object {
        desc: String,
    },
}

impl<'de> Deserialize<'de> for PropDef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = PropSchema::deserialize(deserializer)?;

        match schema.typ.as_str() {
            "string" => {
                let enum_values = match schema.enum_values {
                    Some(values) => {
                        let mut strings = Vec::with_capacity(values.len());
                        for value in values {
                            let string = value.as_str().ok_or_else(|| {
                                D::Error::custom("string enum must contain strings")
                            })?;
                            strings.push(string.to_string());
                        }
                        Some(strings)
                    }
                    None => None,
                };
                Ok(Self::String {
                    desc: schema.desc,
                    r#enum: enum_values,
                })
            }
            "number" => {
                let enum_values = match schema.enum_values {
                    Some(values) => {
                        let mut numbers = Vec::with_capacity(values.len());
                        for value in values {
                            let number = value.as_f64().ok_or_else(|| {
                                D::Error::custom("number enum must contain numbers")
                            })?;
                            numbers.push(number);
                        }
                        Some(numbers)
                    }
                    None => None,
                };
                Ok(Self::Number {
                    desc: schema.desc,
                    r#enum: enum_values,
                })
            }
            "boolean" => Ok(Self::Boolean { desc: schema.desc }),
            "array" => Ok(Self::Array { desc: schema.desc }),
            "object" => Ok(Self::Object { desc: schema.desc }),
            other => Err(D::Error::custom(format!(
                "unsupported property type: {other}"
            ))),
        }
    }
}

impl PropDef {
    fn description(&self) -> &str {
        match self {
            PropDef::String { desc, .. }
            | PropDef::Number { desc, .. }
            | PropDef::Boolean { desc }
            | PropDef::Array { desc }
            | PropDef::Object { desc } => desc,
        }
    }

    fn typ(&self) -> &str {
        match self {
            PropDef::String { .. } => "string",
            PropDef::Number { .. } => "number",
            PropDef::Boolean { .. } => "boolean",
            PropDef::Array { .. } => "array",
            PropDef::Object { .. } => "object",
        }
    }

    fn enum_values(&self) -> Option<serde_json::Value> {
        match self {
            PropDef::String { r#enum, .. } => r#enum.as_ref().map(|v| {
                serde_json::Value::Array(
                    v.iter()
                        .map(|e| serde_json::Value::String(e.clone()))
                        .collect(),
                )
            }),
            PropDef::Number { r#enum, .. } => r#enum.as_ref().map(|v| {
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

#[derive(Debug, Clone, Deserialize)]
pub struct ParamDef {
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(default, rename = "properties")]
    pub props: HashMap<String, PropDef>,
    #[serde(default)]
    pub required: Option<Vec<String>>,
    #[serde(default, rename = "additionalProperties")]
    pub additional_props: Option<bool>,
}

impl ParamDef {
    pub fn new(r#type: &str) -> Self {
        Self {
            r#type: r#type.to_string(),
            props: HashMap::new(),
            required: None,
            additional_props: None,
        }
    }

    /// Add multiple named properties at once.
    ///
    /// Each entry is `(name, property)`. Use [`with_required`](Self::with_required)
    /// to mark specific properties as required.
    pub fn with_properties(mut self, properties: Vec<(&str, PropDef)>) -> Self {
        properties.into_iter().for_each(|(name, prop)| {
            self.props.insert(name.to_string(), prop);
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
        for (name, prop) in &self.props {
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

        if let Some(additional) = self.additional_props {
            map.insert(
                "additionalProperties".to_string(),
                serde_json::Value::Bool(additional),
            );
        }

        serde_json::Value::Object(map)
    }
}

impl Default for ParamDef {
    fn default() -> Self {
        Self {
            r#type: "object".to_string(),
            props: HashMap::new(),
            required: None,
            additional_props: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ToolDefination {
    pub name: String,
    pub description: String,
    pub parameters: ParamDef,
    #[serde(default)]
    pub strict: Option<bool>,
}

impl ToolDefination {
    pub fn new(name: &str, description: &str, parameters: ParamDef) -> Self {
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

/// IToolCall represents a call to a tool, with its name and arguments.
pub trait IToolCall {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn args(&self) -> &str;
}
