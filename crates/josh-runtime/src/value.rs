use std::{collections::BTreeMap, fmt, sync::Arc};

use josh_syntax::{BindingPattern, FunctionBody};

use crate::host::StageOutcome;

pub(crate) type Frame = BTreeMap<String, Value>;

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Arc<str>),
    Bytes(Arc<[u8]>),
    Array(Arc<Vec<Value>>),
    Object(Arc<ObjectValue>),
    #[doc(hidden)]
    Environment,
    Function(Arc<FunctionValue>),
    Error(Arc<ErrorValue>),
    Status(Arc<StatusValue>),
}

impl Value {
    #[must_use]
    pub fn truthy(&self) -> bool {
        match self {
            Self::Null | Self::Bool(false) => false,
            Self::Int(value) => *value != 0,
            Self::Float(value) => *value != 0.0,
            Self::String(value) => !value.is_empty(),
            Self::Bytes(value) => !value.is_empty(),
            Self::Array(value) => !value.is_empty(),
            Self::Object(value) => !value.is_empty(),
            Self::Status(value) => value.success(),
            Self::Bool(true) | Self::Environment | Self::Function(_) | Self::Error(_) => true,
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Bytes(_) => "bytes",
            Self::Array(_) => "array",
            Self::Object(_) | Self::Environment => "object",
            Self::Function(_) => "function",
            Self::Error(_) => "error",
            Self::Status(_) => "status",
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Int(left), Self::Int(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (Self::String(left), Self::String(right)) => left == right,
            (Self::Bytes(left), Self::Bytes(right)) => left == right,
            (Self::Array(left), Self::Array(right)) => left == right,
            (Self::Object(left), Self::Object(right)) => left == right,
            (Self::Environment, Self::Environment) => true,
            (Self::Function(left), Self::Function(right)) => match (&left.kind, &right.kind) {
                (FunctionKind::Builtin(left), FunctionKind::Builtin(right)) => left == right,
                _ => Arc::ptr_eq(left, right),
            },
            (Self::Error(left), Self::Error(right)) => left == right,
            (Self::Status(left), Self::Status(right)) => left == right,
            _ => false,
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("Null"),
            Self::Bool(value) => formatter.debug_tuple("Bool").field(value).finish(),
            Self::Int(value) => formatter.debug_tuple("Int").field(value).finish(),
            Self::Float(value) => formatter.debug_tuple("Float").field(value).finish(),
            Self::String(value) => formatter.debug_tuple("String").field(value).finish(),
            Self::Bytes(value) => formatter.debug_tuple("Bytes").field(value).finish(),
            Self::Array(value) => formatter.debug_tuple("Array").field(value).finish(),
            Self::Object(value) => formatter.debug_tuple("Object").field(value).finish(),
            Self::Environment => formatter.write_str("Environment"),
            Self::Function(value) => formatter.debug_tuple("Function").field(value).finish(),
            Self::Error(value) => formatter.debug_tuple("Error").field(value).finish(),
            Self::Status(value) => formatter.debug_tuple("Status").field(value).finish(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("null"),
            Self::Bool(value) => write!(formatter, "{value}"),
            Self::Int(value) => write!(formatter, "{value}"),
            Self::Float(value) => write!(formatter, "{value}"),
            Self::String(value) => formatter.write_str(value),
            Self::Bytes(value) => write!(formatter, "<{} bytes>", value.len()),
            Self::Array(values) => {
                formatter.write_str("[")?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "{value}")?;
                }
                formatter.write_str("]")
            }
            Self::Object(object) => write!(formatter, "{object}"),
            Self::Environment => formatter.write_str("<environment>"),
            Self::Function(function) => write!(formatter, "{function}"),
            Self::Error(error) => write!(formatter, "{error}"),
            Self::Status(status) => write!(formatter, "{status}"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ObjectValue {
    entries: Vec<(Arc<str>, Value)>,
}

impl ObjectValue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = (Arc<str>, Value)>) -> Self {
        let mut object = Self::new();
        for (key, value) in entries {
            object.insert(key, value);
        }
        object
    }

    pub fn insert(&mut self, key: Arc<str>, value: Value) {
        if let Some((_, current)) = self.entries.iter_mut().find(|(name, _)| name == &key) {
            *current = value;
        } else {
            self.entries.push((key, value));
        }
    }

    pub fn try_insert(
        &mut self,
        key: Arc<str>,
        value: Value,
    ) -> Result<(), std::collections::TryReserveError> {
        if let Some((_, current)) = self.entries.iter_mut().find(|(name, _)| name == &key) {
            *current = value;
        } else {
            self.entries.try_reserve(1)?;
            self.entries.push((key, value));
        }
        Ok(())
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find_map(|(name, value)| (&**name == key).then_some(value))
    }

    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Arc<str>, &Value)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }
}

impl fmt::Display for ObjectValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("{")?;
        for (index, (key, value)) in self.entries.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{key}: {value}")?;
        }
        formatter.write_str("}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorValue {
    kind: Arc<str>,
    message: Arc<str>,
    status: Option<StatusValue>,
}

impl ErrorValue {
    #[must_use]
    pub fn new(kind: impl Into<Arc<str>>, message: impl Into<Arc<str>>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            status: None,
        }
    }

    #[must_use]
    pub fn with_status(
        kind: impl Into<Arc<str>>,
        message: impl Into<Arc<str>>,
        status: StatusValue,
    ) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            status: Some(status),
        }
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn status(&self) -> Option<&StatusValue> {
        self.status.as_ref()
    }
}

impl fmt::Display for ErrorValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.kind, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusValue {
    outcomes: Arc<Vec<StageOutcome>>,
}

impl StatusValue {
    #[must_use]
    pub fn new(outcomes: Vec<StageOutcome>) -> Self {
        Self {
            outcomes: Arc::new(outcomes),
        }
    }

    #[must_use]
    pub fn success(&self) -> bool {
        self.outcomes.iter().all(|outcome| outcome.success)
    }

    #[must_use]
    pub fn code(&self) -> i32 {
        self.outcomes.last().map_or(0, |outcome| {
            outcome.code.unwrap_or_else(|| {
                outcome
                    .signal
                    .map_or(1, |signal| 128_i32.saturating_add(signal))
            })
        })
    }

    #[must_use]
    pub fn outcomes(&self) -> &[StageOutcome] {
        &self.outcomes
    }
}

impl fmt::Display for StatusValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "status {}", self.code())
    }
}

#[derive(Clone)]
pub struct FunctionValue {
    pub(crate) kind: FunctionKind,
}

#[derive(Clone)]
pub(crate) enum FunctionKind {
    User {
        name: Option<Arc<str>>,
        params: Arc<Vec<BindingPattern>>,
        body: Arc<FunctionBody>,
        captures: Arc<Frame>,
    },
    Builtin(BuiltinFunction),
}

impl FunctionValue {
    pub(crate) fn user(
        name: Option<Arc<str>>,
        params: Vec<BindingPattern>,
        body: FunctionBody,
        captures: Frame,
    ) -> Self {
        Self {
            kind: FunctionKind::User {
                name,
                params: Arc::new(params),
                body: Arc::new(body),
                captures: Arc::new(captures),
            },
        }
    }

    pub(crate) const fn builtin(builtin: BuiltinFunction) -> Self {
        Self {
            kind: FunctionKind::Builtin(builtin),
        }
    }

    fn name(&self) -> &str {
        match &self.kind {
            FunctionKind::User { name, .. } => name.as_deref().unwrap_or("anonymous"),
            FunctionKind::Builtin(builtin) => builtin.name(),
        }
    }
}

impl fmt::Debug for FunctionValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FunctionValue")
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for FunctionValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "<function {}>", self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinFunction {
    String,
    Int,
    Float,
    Bool,
    Error,
    Glob,
}

impl BuiltinFunction {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Error => "error",
            Self::Glob => "glob",
        }
    }
}
