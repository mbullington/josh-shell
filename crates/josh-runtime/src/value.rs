use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, RwLock},
};

use josh_syntax::{BindingPattern, FunctionBody};

use crate::host::StageOutcome;

const FRAME_SPILL: usize = 8;

/// Binding frame: linear-scanned pair list until it outgrows `FRAME_SPILL`,
/// then spills into an ordered map. Most frames (call args, block scopes)
/// hold only a few names.
#[derive(Clone)]
pub(crate) enum Frame {
    Small(Vec<(String, Value)>),
    Map(BTreeMap<String, Value>),
}

impl Frame {
    pub(crate) fn new() -> Self {
        Self::Small(Vec::new())
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Value> {
        match self {
            Self::Small(entries) => entries
                .iter()
                .rev()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value),
            Self::Map(entries) => entries.get(name),
        }
    }

    pub(crate) fn contains_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub(crate) fn insert(&mut self, name: String, value: Value) {
        match self {
            Self::Small(entries) => {
                if let Some(slot) = entries.iter_mut().rev().find(|(key, _)| *key == name) {
                    slot.1 = value;
                } else if entries.len() >= FRAME_SPILL {
                    let mut map = BTreeMap::from_iter(entries.drain(..));
                    map.insert(name, value);
                    *self = Self::Map(map);
                } else {
                    entries.push((name, value));
                }
            }
            Self::Map(entries) => {
                entries.insert(name, value);
            }
        }
    }

    pub(crate) fn extend(&mut self, bindings: impl IntoIterator<Item = (String, Value)>) {
        for (name, value) in bindings {
            self.insert(name, value);
        }
    }

    pub(crate) fn keys(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        match self {
            Self::Small(entries) => Box::new(entries.iter().map(|(name, _)| name)),
            Self::Map(entries) => Box::new(entries.keys()),
        }
    }

    pub(crate) fn iter(&self) -> Box<dyn Iterator<Item = (&String, &Value)> + '_> {
        match self {
            Self::Small(entries) => Box::new(entries.iter().map(|(name, value)| (name, value))),
            Self::Map(entries) => Box::new(entries.iter()),
        }
    }
}

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Arc<str>),
    Bytes(Arc<[u8]>),
    Array(Arc<ArrayValue>),
    Object(Arc<ObjectValue>),
    #[doc(hidden)]
    Environment,
    Function(Arc<FunctionValue>),
    Error(Arc<ErrorValue>),
    Status(Arc<StatusValue>),
}

impl Value {
    #[must_use]
    pub fn array(items: Vec<Value>) -> Self {
        Self::Array(Arc::new(ArrayValue::from_vec(items)))
    }

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
                (FunctionKind::Native(left), FunctionKind::Native(right)) => {
                    std::ptr::eq(*left, *right)
                }
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
                for (index, value) in values.snapshot().iter().enumerate() {
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

/// A Josh array: mutable, shared `Arc<ArrayValue>` handles observe the same
/// interior state, so `push`/`pop`/`reverse`/`sort` edit in place
/// JavaScript-style. Read paths that invoke callbacks snapshot first.
#[derive(Debug, Default)]
pub struct ArrayValue {
    items: RwLock<Vec<Value>>,
}

impl ArrayValue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_vec(items: Vec<Value>) -> Self {
        Self {
            items: RwLock::new(items),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<Value> {
        self.items
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get(index)
            .cloned()
    }

    /// Clone `items[lo..hi]` under a single read lock; bounds must already
    /// be clamped to the array length.
    #[must_use]
    pub fn slice_range(&self, lo: usize, hi: usize) -> Vec<Value> {
        let items = self.items.read().unwrap_or_else(|error| error.into_inner());
        items[lo..hi.min(items.len()).max(lo)].to_vec()
    }

    /// Signed-index access (`a[-1]` counts from the end) under one lock.
    #[must_use]
    pub fn get_indexed(&self, index: i64) -> Option<Value> {
        let items = self.items.read().unwrap_or_else(|error| error.into_inner());
        let Ok(index) = usize::try_from(if index < 0 {
            index + items.len() as i64
        } else {
            index
        }) else {
            return None;
        };
        items.get(index).cloned()
    }

    /// A snapshot of the current items; iteration order is stable.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Value> {
        self.items
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// Append values, returning the new length (Array.prototype.push).
    pub fn push_many(&self, values: impl IntoIterator<Item = Value>) -> usize {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(|error| error.into_inner());
        items.extend(values);
        items.len()
    }

    /// Remove and return the last item (Array.prototype.pop).
    pub fn pop(&self) -> Option<Value> {
        self.items
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .pop()
    }

    /// Exclusive access for in-place transforms (reverse/sort); callbacks run
    /// while the write lock is held, so they must not re-enter the array.
    pub fn with_mut<R>(&self, transform: impl FnOnce(&mut Vec<Value>) -> R) -> R {
        transform(
            &mut self
                .items
                .write()
                .unwrap_or_else(|error| error.into_inner()),
        )
    }
}

impl PartialEq for ArrayValue {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        self.snapshot() == other.snapshot()
    }
}

#[derive(Debug, Default)]
pub(crate) struct ObjectState {
    pub entries: Vec<(Arc<str>, Value)>,
    pub prototype: Option<Value>,
    pub sealed: bool,
}

/// A Josh object: mutable, optionally sealed, with an optional prototype
/// value. Shared `Arc<ObjectValue>` handles observe the same interior state.
#[derive(Debug, Default)]
pub struct ObjectValue {
    state: RwLock<ObjectState>,
}

impl ObjectValue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = (Arc<str>, Value)>) -> Self {
        let object = Self::new();
        for (key, value) in entries {
            object.insert(key, value);
        }
        object
    }

    pub fn insert(&self, key: Arc<str>, value: Value) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((_, current)) = state.entries.iter_mut().find(|(name, _)| name == &key) {
            *current = value;
        } else {
            state.entries.push((key, value));
        }
    }

    pub fn try_insert(
        &self,
        key: Arc<str>,
        value: Value,
    ) -> Result<(), std::collections::TryReserveError> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((_, current)) = state.entries.iter_mut().find(|(name, _)| name == &key) {
            *current = value;
        } else {
            state.entries.try_reserve(1)?;
            state.entries.push((key, value));
        }
        Ok(())
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<Value> {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .entries
            .iter()
            .find_map(|(name, value)| (&**name == key).then(|| value.clone()))
    }

    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .entries
            .iter()
            .any(|(name, _)| &**name == key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .entries
            .len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A snapshot of the object's own entries in insertion order.
    #[must_use]
    pub fn snapshot(&self) -> Vec<(Arc<str>, Value)> {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .entries
            .clone()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Arc<str>, Value)> {
        self.snapshot().into_iter()
    }

    #[must_use]
    pub fn prototype(&self) -> Option<Value> {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .prototype
            .clone()
    }

    pub fn set_prototype(&self, prototype: Option<Value>) {
        self.state
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .prototype = prototype;
    }

    #[must_use]
    pub fn sealed(&self) -> bool {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .sealed
    }

    pub fn set_sealed(&self, sealed: bool) {
        self.state
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .sealed = sealed;
    }

    /// Write a member honoring the sealed rule: existing members stay
    /// writable, new members are rejected on sealed objects.
    pub fn assign_member(&self, key: Arc<str>, value: Value) -> Result<(), String> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((_, current)) = state.entries.iter_mut().find(|(name, _)| name == &key) {
            *current = value;
            return Ok(());
        }
        if state.sealed {
            return Err(format!("object is sealed; cannot add member `{key}`"));
        }
        state.entries.push((key, value));
        Ok(())
    }

    #[must_use]
    pub fn own_entries(&self) -> Vec<(Arc<str>, Value)> {
        self.snapshot()
    }
}

impl PartialEq for ObjectValue {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        self.snapshot() == other.snapshot()
    }
}

impl fmt::Display for ObjectValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("{")?;
        for (index, (key, value)) in self.snapshot().iter().enumerate() {
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
    pub(crate) members: Option<Value>,
}

#[derive(Clone)]
pub(crate) enum FunctionKind {
    User {
        name: Option<Arc<str>>,
        params: Arc<Vec<BindingPattern>>,
        body: Arc<FunctionBody>,
        captures: Arc<Frame>,
    },
    Native(&'static NativeFn),
}

/// A function implemented in Rust. Receives the engine and already-evaluated
/// arguments, like a builtin.
pub struct NativeFn {
    pub name: &'static str,
    pub function: fn(&mut crate::engine::Engine, Vec<Value>) -> crate::engine::EvalResult<Value>,
}

impl fmt::Debug for NativeFn {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeFn")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
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
            members: None,
        }
    }

    pub(crate) const fn native(native: &'static NativeFn) -> Self {
        Self {
            kind: FunctionKind::Native(native),
            members: None,
        }
    }

    pub(crate) fn native_with_members(native: &'static NativeFn, members: Value) -> Self {
        Self {
            kind: FunctionKind::Native(native),
            members: Some(members),
        }
    }

    pub(crate) fn member(&self, name: &str) -> Option<Value> {
        match self.members.as_ref()? {
            Value::Object(object) => object.get(name),
            _ => None,
        }
    }

    fn name(&self) -> &str {
        match &self.kind {
            FunctionKind::User { name, .. } => name.as_deref().unwrap_or("anonymous"),
            FunctionKind::Native(native) => native.name,
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
