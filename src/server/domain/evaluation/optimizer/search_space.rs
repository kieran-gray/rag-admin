use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
}

impl Value {
    pub fn as_string(&self) -> Option<&str> {
        if let Value::String(s) = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(n) = self {
            Some(*n)
        } else {
            None
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parameter {
    Categorical {
        name: String,
        values: Vec<String>,
    },
    IntRange {
        name: String,
        low: i64,
        high: i64,
        log_scale: bool,
    },
    Float {
        name: String,
        low: f64,
        high: f64,
        log_scale: bool,
    },

    Conditional {
        gate_parameter: String,
        gate_value: Value,
        inner: Box<Parameter>,
    },
}

impl Parameter {
    pub fn name(&self) -> &str {
        match self {
            Parameter::Categorical { name, .. } => name,
            Parameter::IntRange { name, .. } => name,
            Parameter::Float { name, .. } => name,
            Parameter::Conditional { inner, .. } => inner.name(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchSpace {
    parameters: Vec<Parameter>,
}

impl SearchSpace {
    pub fn new(parameters: Vec<Parameter>) -> Self {
        Self { parameters }
    }

    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    pub fn active_parameters<'a>(&'a self, params: &HashMap<String, Value>) -> Vec<&'a Parameter> {
        self.parameters
            .iter()
            .filter(|p| match p {
                Parameter::Conditional {
                    gate_parameter,
                    gate_value,
                    ..
                } => params.get(gate_parameter) == Some(gate_value),
                _ => true,
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Trial {
    pub trial_id: u32,
    pub params: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct Observation {
    pub trial_id: u32,
    pub params: HashMap<String, Value>,
    pub fitness: Fitness,
}

#[derive(Debug, Clone)]
pub struct Fitness {
    pub composite: f32,
    pub composite_ci: (f32, f32),
    pub recall: f32,
    pub precision: f32,
    pub iou: f32,
    pub precision_omega: f32,
    pub cost: f32,

    pub judge_score: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_parameters_filter_by_gate() {
        let space = SearchSpace::new(vec![
            Parameter::Categorical {
                name: "strategy".into(),
                values: vec!["section".into(), "bert".into()],
            },
            Parameter::Conditional {
                gate_parameter: "strategy".into(),
                gate_value: Value::String("section".into()),
                inner: Box::new(Parameter::IntRange {
                    name: "section_tokens".into(),
                    low: 100,
                    high: 1000,
                    log_scale: false,
                }),
            },
        ]);

        let mut params = HashMap::new();
        params.insert("strategy".into(), Value::String("section".into()));
        let active: Vec<&str> = space
            .active_parameters(&params)
            .iter()
            .map(|p| p.name())
            .collect();
        assert!(active.contains(&"strategy"));
        assert!(active.contains(&"section_tokens"));

        params.insert("strategy".into(), Value::String("bert".into()));
        let active: Vec<&str> = space
            .active_parameters(&params)
            .iter()
            .map(|p| p.name())
            .collect();
        assert!(active.contains(&"strategy"));
        assert!(!active.contains(&"section_tokens"));
    }
}
