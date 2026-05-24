use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub desc: String,
    pub params: ParamDef,
    pub strict: Option<bool>,
}

/// Builder for [`ToolDef`].
pub struct ToolDefBuilder {
    name: String,
    desc: String,
    params: ParamDef,
    strict: Option<bool>,
}

impl ToolDefBuilder {
    pub fn new(name: String, desc: String, params: ParamDef) -> Self {
        Self {
            name,
            desc,
            params,
            strict: None,
        }
    }

    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = Some(strict);
        self
    }

    pub fn build(self) -> ToolDef {
        ToolDef {
            name: self.name,
            desc: self.desc,
            params: self.params,
            strict: self.strict,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParamDef {
    pub r#type: String,
    pub props: HashMap<String, PropDef>,
    pub required: Option<Vec<String>>,
    pub additional_props: Option<bool>,
}

impl ParamDef {
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
                serde_json::Value::String(prop.desc().to_string()),
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

/// Builder for [`ParamDef`].
pub struct ParamDefBuilder {
    r#type: String,
    props: HashMap<String, PropDef>,
    required: Option<Vec<String>>,
    additional_props: Option<bool>,
}

impl ParamDefBuilder {
    pub fn new(r#type: String) -> Self {
        Self {
            r#type,
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

    pub fn build(self) -> ParamDef {
        ParamDef {
            r#type: self.r#type,
            props: self.props,
            required: self.required,
            additional_props: self.additional_props,
        }
    }
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

impl PropDef {
    fn desc(&self) -> &str {
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

/// IToolCall represents a call to a tool, with its name and arguments.
pub trait IToolCall {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn args(&self) -> &str;
}
