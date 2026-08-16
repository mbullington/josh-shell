//! Josh's prototypal builtin namespaces.
//!
//! `Object`, `String`, `Number`, `Boolean`, `Array`, and `Function` are
//! callable native functions that also carry members (most importantly
//! `.prototype`). Type prototypes are ordinary objects chained to the root
//! `Object.prototype`; prototype functions are invoked with the receiver as
//! their first argument (Python-style), and `error`/`glob` stay plain.

use std::sync::Arc;

use crate::engine::{
    Engine, EvalResult, bytes_to_value, expect_arity, expect_int, expect_string, flatten,
    scalar_to_string, sequence_at, string_at, type_error, usize_value,
};
use crate::value::{ErrorValue, Frame, FunctionValue, NativeFn, ObjectValue, Value};

/// Per-engine type prototype values, terminal-rooted at `Object.prototype`.
pub(crate) struct Prototypes {
    pub root: Value,
    pub string: Value,
    pub number: Value,
    pub boolean: Value,
    pub array: Value,
    pub function: Value,
}

impl Prototypes {
    pub(crate) fn for_value(&self, value: &Value) -> Option<Value> {
        match value {
            Value::Object(object) => object.prototype(),
            Value::String(_) => Some(self.string.clone()),
            Value::Int(_) | Value::Float(_) => Some(self.number.clone()),
            Value::Bool(_) => Some(self.boolean.clone()),
            Value::Array(_) => Some(self.array.clone()),
            Value::Function(_) => Some(self.function.clone()),
            _ => None,
        }
    }
}

fn native(name: &'static str, function: fn(&mut Engine, Vec<Value>) -> EvalResult<Value>) -> Value {
    Value::Function(Arc::new(FunctionValue::native(&NativeFnBox::leak(
        name, function,
    ))))
}

fn object(entries: impl IntoIterator<Item = (Arc<str>, Value)>) -> Arc<ObjectValue> {
    Arc::new(ObjectValue::from_entries(entries))
}

fn chained(prototype: &Value, entries: impl IntoIterator<Item = (Arc<str>, Value)>) -> Value {
    let object = object(entries);
    object.set_prototype(Some(prototype.clone()));
    Value::Object(object)
}

/// Install the builtin namespaces into the engine's root frame.
pub(crate) fn install(frame: &mut Frame) -> Prototypes {
    let root = Value::Object(object([]));

    let string_prototype = chained(
        &root,
        [
            (
                Arc::from("contains"),
                native("String.prototype.contains", string_contains),
            ),
            (
                Arc::from("includes"),
                native("String.prototype.contains", string_contains),
            ),
            (
                Arc::from("startsWith"),
                native("String.prototype.startsWith", string_starts_with),
            ),
            (
                Arc::from("endsWith"),
                native("String.prototype.endsWith", string_ends_with),
            ),
            (
                Arc::from("split"),
                native("String.prototype.split", string_split),
            ),
            (
                Arc::from("replace"),
                native("String.prototype.replace", string_replace),
            ),
            (
                Arc::from("replaceAll"),
                native("String.prototype.replaceAll", string_replace_all),
            ),
            (
                Arc::from("trim"),
                native("String.prototype.trim", string_trim),
            ),
            (
                Arc::from("toUpperCase"),
                native("String.prototype.toUpperCase", string_to_upper),
            ),
            (
                Arc::from("toLowerCase"),
                native("String.prototype.toLowerCase", string_to_lower),
            ),
            (
                Arc::from("at"),
                native("String.prototype.at", string_at_method),
            ),
        ],
    );
    let number_prototype = chained(
        &root,
        [
            (
                Arc::from("ceil"),
                native("Number.prototype.ceil", number_ceil),
            ),
            (
                Arc::from("floor"),
                native("Number.prototype.floor", number_floor),
            ),
            (
                Arc::from("round"),
                native("Number.prototype.round", number_round),
            ),
            (Arc::from("abs"), native("Number.prototype.abs", number_abs)),
            (
                Arc::from("norm"),
                native("Number.prototype.norm", number_norm),
            ),
        ],
    );
    let array_prototype = chained(
        &root,
        [
            (Arc::from("at"), native("Array.prototype.at", array_at)),
            (
                Arc::from("contains"),
                native("Array.prototype.contains", array_contains),
            ),
            (
                Arc::from("includes"),
                native("Array.prototype.contains", array_contains),
            ),
            (Arc::from("map"), native("Array.prototype.map", array_map)),
            (
                Arc::from("filter"),
                native("Array.prototype.filter", array_filter),
            ),
            (
                Arc::from("reduce"),
                native("Array.prototype.reduce", array_reduce),
            ),
            (
                Arc::from("flat"),
                native("Array.prototype.flat", array_flat),
            ),
            (
                Arc::from("join"),
                native("Array.prototype.join", array_join),
            ),
            (
                Arc::from("slice"),
                native("Array.prototype.slice", array_slice),
            ),
        ],
    );
    let boolean_prototype = chained(&root, []);
    let function_prototype = chained(&root, []);

    let object_namespace = Value::Function(Arc::new(FunctionValue::native_with_members(
        leak("Object", object_convert),
        Value::Object(object([
            (Arc::from("prototype"), root.clone()),
            (Arc::from("keys"), native("Object.keys", object_keys)),
            (
                Arc::from("entries"),
                native("Object.entries", object_entries),
            ),
            (Arc::from("values"), native("Object.values", object_values)),
            (Arc::from("create"), native("Object.create", object_create)),
            (
                Arc::from("fromEntries"),
                native("Object.fromEntries", object_from_entries),
            ),
            (
                Arc::from("getPrototype"),
                native("Object.getPrototype", object_get_prototype),
            ),
            (
                Arc::from("setPrototype"),
                native("Object.setPrototype", object_set_prototype),
            ),
            (Arc::from("seal"), native("Object.seal", object_seal)),
            (
                Arc::from("isSealed"),
                native("Object.isSealed", object_is_sealed),
            ),
        ])),
    )));
    let string_namespace = Value::Function(Arc::new(FunctionValue::native_with_members(
        leak("String", string_convert),
        Value::Object(object([(Arc::from("prototype"), string_prototype.clone())])),
    )));
    let number_namespace = Value::Function(Arc::new(FunctionValue::native_with_members(
        leak("Number", number_convert),
        Value::Object(object([
            (Arc::from("prototype"), number_prototype.clone()),
            (Arc::from("NaN"), Value::Float(f64::NAN)),
            (Arc::from("MAX_VALUE"), Value::Float(f64::MAX)),
            (Arc::from("MIN_VALUE"), Value::Float(-f64::MAX)),
            (Arc::from("MAX_INT"), Value::Int(i64::MAX)),
            (Arc::from("MIN_INT"), Value::Int(i64::MIN)),
        ])),
    )));
    let boolean_namespace = Value::Function(Arc::new(FunctionValue::native_with_members(
        leak("Boolean", boolean_convert),
        Value::Object(object([(
            Arc::from("prototype"),
            boolean_prototype.clone(),
        )])),
    )));
    let array_namespace = Value::Function(Arc::new(FunctionValue::native_with_members(
        leak("Array", array_convert),
        Value::Object(object([(Arc::from("prototype"), array_prototype.clone())])),
    )));
    let function_namespace = Value::Function(Arc::new(FunctionValue::native_with_members(
        leak("Function", function_convert),
        Value::Object(object([(
            Arc::from("prototype"),
            function_prototype.clone(),
        )])),
    )));

    frame.insert("Object".into(), object_namespace);
    frame.insert("String".into(), string_namespace);
    frame.insert("Number".into(), number_namespace);
    frame.insert("Boolean".into(), boolean_namespace);
    frame.insert("Array".into(), array_namespace);
    frame.insert("Function".into(), function_namespace);
    frame.insert("error".into(), native("error", error_construct));
    frame.insert("glob".into(), native("glob", glob_expand));

    Prototypes {
        root,
        string: string_prototype,
        number: number_prototype,
        boolean: boolean_prototype,
        array: array_prototype,
        function: function_prototype,
    }
}

// `NativeFn` needs a `'static' address; type namespaces are small and built
// once per engine without dropping the boxes.
struct NativeFnBox;

impl NativeFnBox {
    fn leak(
        name: &'static str,
        function: fn(&mut Engine, Vec<Value>) -> EvalResult<Value>,
    ) -> &'static NativeFn {
        Box::leak(Box::new(NativeFn { name, function }))
    }
}

fn leak(
    name: &'static str,
    function: fn(&mut Engine, Vec<Value>) -> EvalResult<Value>,
) -> &'static NativeFn {
    NativeFnBox::leak(name, function)
}

// --- Conversions -----------------------------------------------------------

fn string_convert(_engine: &mut Engine, mut args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("String", &args, 1, 1)?;
    let value = args.pop().expect("arity checked");
    scalar_to_string(&value).map(|value| Value::String(value.into()))
}

fn number_convert(_engine: &mut Engine, mut args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Number", &args, 1, 1)?;
    let value = args.pop().expect("arity checked");
    match value {
        Value::Int(_) | Value::Float(_) => Ok(value),
        Value::Bool(value) => Ok(Value::Int(i64::from(value))),
        Value::String(text) => text
            .parse::<i64>()
            .map(Value::Int)
            .or_else(|_| text.parse::<f64>().map(Value::Float))
            .map_err(|_| type_error("string is not numeric")),
        value => Err(type_error(format!(
            "cannot convert {} to number",
            value.type_name()
        ))),
    }
}

fn boolean_convert(_engine: &mut Engine, mut args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Boolean", &args, 1, 1)?;
    let value = args.pop().expect("arity checked");
    Ok(Value::Bool(value.truthy()))
}

fn array_convert(_engine: &mut Engine, mut args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Array", &args, 1, 1)?;
    let value = args.pop().expect("arity checked");
    Ok(match value {
        Value::Array(_) => value,
        Value::Null => Value::Array(Arc::new(Vec::new())),
        value => Value::Array(Arc::new(vec![value])),
    })
}

fn object_convert(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Object", &args, 0, 0)?;
    Err(type_error(
        "Object is not callable; build objects with `{...}`",
    ))
}

fn function_convert(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Function", &args, 0, 0)?;
    Err(type_error(
        "Function is not callable; build functions with `=>`",
    ))
}

fn error_construct(_engine: &mut Engine, mut args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("error", &args, 1, 1)?;
    let value = args.pop().expect("arity checked");
    scalar_to_string(&value).map(|message| Value::Error(Arc::new(ErrorValue::new("user", message))))
}

fn glob_expand(engine: &mut Engine, mut args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("glob", &args, 1, 1)?;
    let value = args.pop().expect("arity checked");
    let pattern = expect_string(&value)?;
    let matches = engine
        .host()
        .glob(pattern.as_bytes(), engine.shell_context_shared())
        .map_err(crate::EngineError::from)?;
    Ok(Value::Array(Arc::new(
        matches.into_iter().map(bytes_to_value).collect(),
    )))
}

// --- String.prototype ------------------------------------------------------

fn string_receiver<'a>(
    name: &'static str,
    args: &'a [Value],
    minimum: usize,
    maximum: usize,
) -> EvalResult<&'a str> {
    expect_arity(name, args, minimum + 1, maximum + 1)?;
    let Value::String(value) = &args[0] else {
        return Err(type_error(format!(
            "{name} receiver must be a string, got {}",
            args[0].type_name()
        )));
    };
    Ok(value)
}

fn string_contains(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.contains", &args, 1, 1)?;
    Ok(Value::Bool(receiver.contains(expect_string(&args[1])?)))
}

fn string_starts_with(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.startsWith", &args, 1, 1)?;
    Ok(Value::Bool(receiver.starts_with(expect_string(&args[1])?)))
}

fn string_ends_with(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.endsWith", &args, 1, 1)?;
    Ok(Value::Bool(receiver.ends_with(expect_string(&args[1])?)))
}

fn string_split(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.split", &args, 1, 1)?;
    let separator = expect_string(&args[1])?;
    let parts: Vec<String> = if separator.is_empty() {
        receiver.chars().map(|ch| ch.to_string()).collect()
    } else {
        receiver.split(separator).map(str::to_owned).collect()
    };
    Ok(Value::Array(Arc::new(
        parts
            .into_iter()
            .map(|part| Value::String(Arc::from(part)))
            .collect(),
    )))
}

fn string_replace(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.replace", &args, 2, 2)?;
    let from = expect_string(&args[1])?;
    let to = expect_string(&args[2])?;
    Ok(Value::String(Arc::from(receiver.replacen(from, to, 1))))
}

fn string_replace_all(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.replaceAll", &args, 2, 2)?;
    let from = expect_string(&args[1])?;
    let to = expect_string(&args[2])?;
    Ok(Value::String(Arc::from(receiver.replace(from, to))))
}

fn string_trim(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.trim", &args, 0, 0)?;
    Ok(Value::String(Arc::from(receiver.trim())))
}

fn string_to_upper(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.toUpperCase", &args, 0, 0)?;
    Ok(Value::String(Arc::from(receiver.to_uppercase())))
}

fn string_to_lower(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.toLowerCase", &args, 0, 0)?;
    Ok(Value::String(Arc::from(receiver.to_lowercase())))
}

fn string_at_method(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = string_receiver("String.prototype.at", &args, 1, 1)?;
    let index = expect_int(&args[1])?;
    Ok(string_at(receiver, index))
}

// --- Number.prototype ------------------------------------------------------

fn number_receiver(
    name: &'static str,
    args: &[Value],
    minimum: usize,
    maximum: usize,
) -> EvalResult<Value> {
    expect_arity(name, args, minimum + 1, maximum + 1)?;
    match &args[0] {
        Value::Int(_) | Value::Float(_) => Ok(args[0].clone()),
        value => Err(type_error(format!(
            "{name} receiver must be a number, got {}",
            value.type_name()
        ))),
    }
}

fn rounded(number: Value, name: &'static str, round: impl Fn(f64) -> f64) -> Value {
    match number {
        Value::Int(value) => Value::Int(value),
        Value::Float(value) => Value::Float(round(value)),
        _ => unreachable!("{name} receiver validated as number"),
    }
}

fn number_ceil(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = number_receiver("Number.prototype.ceil", &args, 0, 0)?;
    Ok(rounded(receiver, "Number.prototype.ceil", f64::ceil))
}

fn number_floor(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = number_receiver("Number.prototype.floor", &args, 0, 0)?;
    Ok(rounded(receiver, "Number.prototype.floor", f64::floor))
}

fn number_round(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = number_receiver("Number.prototype.round", &args, 0, 0)?;
    Ok(rounded(receiver, "Number.prototype.round", f64::round))
}

fn number_abs(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = number_receiver("Number.prototype.abs", &args, 0, 0)?;
    Ok(match receiver {
        Value::Int(value) => Value::Int(value.abs()),
        Value::Float(value) => Value::Float(value.abs()),
        _ => unreachable!(),
    })
}

fn number_norm(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = number_receiver("Number.prototype.norm", &args, 0, 0)?;
    match receiver {
        Value::Float(value)
            if value.is_finite()
                && value.fract() == 0.0
                && value >= i64::MIN as f64
                && value < -(i64::MIN as f64) =>
        {
            Ok(Value::Int(value as i64))
        }
        value => Ok(value),
    }
}

// --- Array.prototype -------------------------------------------------------

fn array_receiver<'a>(
    name: &'static str,
    args: &'a [Value],
    minimum: usize,
    maximum: usize,
) -> EvalResult<&'a [Value]> {
    expect_arity(name, args, minimum + 1, maximum + 1)?;
    let Value::Array(value) = &args[0] else {
        return Err(type_error(format!(
            "{name} receiver must be an array, got {}",
            args[0].type_name()
        )));
    };
    Ok(value)
}

fn array_at(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = array_receiver("Array.prototype.at", &args, 1, 1)?;
    Ok(sequence_at(receiver, expect_int(&args[1])?)
        .cloned()
        .unwrap_or(Value::Null))
}

fn array_contains(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = array_receiver("Array.prototype.contains", &args, 1, 1)?;
    Ok(Value::Bool(receiver.contains(&args[1])))
}

fn array_map(engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = array_receiver("Array.prototype.map", &args, 1, 1)?.to_vec();
    let function = args[1].clone();
    let whole = Value::Array(Arc::new(receiver.clone()));
    let mut output = Vec::with_capacity(receiver.len());
    for (index, item) in receiver.iter().enumerate() {
        output.push(engine.call_value(
            function.clone(),
            vec![item.clone(), usize_value(index)?, whole.clone()],
        )?);
    }
    Ok(Value::Array(Arc::new(output)))
}

fn array_filter(engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = array_receiver("Array.prototype.filter", &args, 1, 1)?.to_vec();
    let function = args[1].clone();
    let whole = Value::Array(Arc::new(receiver.clone()));
    let mut output = Vec::new();
    for (index, item) in receiver.iter().enumerate() {
        let keep = engine.call_value(
            function.clone(),
            vec![item.clone(), usize_value(index)?, whole.clone()],
        )?;
        if keep.truthy() {
            output.push(item.clone());
        }
    }
    Ok(Value::Array(Arc::new(output)))
}

fn array_reduce(engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = array_receiver("Array.prototype.reduce", &args, 1, 2)?.to_vec();
    let function = args[1].clone();
    let (mut accumulator, start) = if let Some(initial) = args.get(2) {
        (initial.clone(), 0)
    } else {
        let Some(first) = receiver.first() else {
            return Err(type_error(
                "reduce on an empty array requires an initial value",
            ));
        };
        (first.clone(), 1)
    };
    let whole = Value::Array(Arc::new(receiver.clone()));
    for (index, item) in receiver.iter().enumerate().skip(start) {
        accumulator = engine.call_value(
            function.clone(),
            vec![
                accumulator,
                item.clone(),
                usize_value(index)?,
                whole.clone(),
            ],
        )?;
    }
    Ok(accumulator)
}

fn array_flat(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = array_receiver("Array.prototype.flat", &args, 0, 1)?;
    let depth = args.get(1).map_or(Ok(1), expect_int)?;
    if depth < 0 {
        return Err(type_error("flat depth must be nonnegative"));
    }
    let mut output = Vec::new();
    flatten(receiver, depth, &mut output);
    Ok(Value::Array(Arc::new(output)))
}

fn array_join(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = array_receiver("Array.prototype.join", &args, 0, 1)?;
    let separator = args.get(1).map_or(Ok(","), expect_string)?;
    let text = receiver
        .iter()
        .map(scalar_to_string)
        .collect::<EvalResult<Vec<_>>>()?
        .join(separator);
    Ok(Value::String(Arc::from(text)))
}

fn array_slice(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let receiver = array_receiver("Array.prototype.slice", &args, 0, 2)?;
    let len =
        i64::try_from(receiver.len()).map_err(|_| type_error("array is too large to index"))?;
    let start = args.get(1).map_or(Ok(0), expect_int)?;
    let end = args.get(2).map_or(Ok(len), expect_int)?;
    let negative_start = start.clamp(-len, len);
    let negative_end = end.clamp(-len, len);
    let start = if negative_start < 0 {
        negative_start + len
    } else {
        negative_start
    };
    let end = if negative_end < 0 {
        negative_end + len
    } else {
        negative_end
    };
    if start >= end {
        return Ok(Value::Array(Arc::new(Vec::new())));
    }
    let (start, end) = (usize::try_from(start), usize::try_from(end));
    let (Ok(start), Ok(end)) = (start, end) else {
        return Err(type_error("array is too large to index"));
    };
    Ok(Value::Array(Arc::new(receiver[start..end].to_vec())))
}

// --- Object statics --------------------------------------------------------

fn object_arg<'a>(
    name: &'static str,
    args: &'a [Value],
    minimum: usize,
    maximum: usize,
) -> EvalResult<&'a ObjectValue> {
    expect_arity(name, args, minimum, maximum)?;
    let Value::Object(object) = &args[0] else {
        return Err(type_error(format!(
            "{name} expects an object, got {}",
            args[0].type_name()
        )));
    };
    Ok(object)
}

fn object_keys(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let object = object_arg("Object.keys", &args, 1, 1)?;
    Ok(Value::Array(Arc::new(
        object
            .snapshot()
            .into_iter()
            .map(|(key, _)| Value::String(key))
            .collect(),
    )))
}

fn object_values(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let object = object_arg("Object.values", &args, 1, 1)?;
    Ok(Value::Array(Arc::new(
        object
            .snapshot()
            .into_iter()
            .map(|(_, value)| value)
            .collect(),
    )))
}

fn object_entries(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let object = object_arg("Object.entries", &args, 1, 1)?;
    Ok(Value::Array(Arc::new(
        object
            .snapshot()
            .into_iter()
            .map(|(key, value)| Value::Array(Arc::new(vec![Value::String(key), value])))
            .collect(),
    )))
}

fn object_create(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Object.create", &args, 1, 1)?;
    let object = ObjectValue::new();
    let prototype = match &args[0] {
        Value::Null => None,
        Value::Object(_) => Some(args[0].clone()),
        value => {
            return Err(type_error(format!(
                "Object.create expects an object or null, got {}",
                value.type_name()
            )));
        }
    };
    object.set_prototype(prototype);
    Ok(Value::Object(Arc::new(object)))
}

fn object_from_entries(engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Object.fromEntries", &args, 1, 1)?;
    let Value::Array(entries) = &args[0] else {
        return Err(type_error(format!(
            "Object.fromEntries expects an array of pairs, got {}",
            args[0].type_name()
        )));
    };
    let object = ObjectValue::new();
    for entry in entries.iter() {
        let Value::Array(pair) = entry else {
            return Err(type_error(
                "Object.fromEntries entries must be [key, value] pairs",
            ));
        };
        let [key, value, ..] = pair.as_slice() else {
            return Err(type_error(
                "Object.fromEntries entries must be [key, value] pairs",
            ));
        };
        let Value::String(key) = key else {
            return Err(type_error("Object.fromEntries keys must be strings"));
        };
        object.insert(Arc::clone(key), value.clone());
    }
    object.set_prototype(Some(engine.type_prototypes().root.clone()));
    Ok(Value::Object(Arc::new(object)))
}

fn object_get_prototype(engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Object.getPrototype", &args, 1, 1)?;
    Ok(engine
        .type_prototypes()
        .for_value(&args[0])
        .unwrap_or(Value::Null))
}

fn object_set_prototype(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    expect_arity("Object.setPrototype", &args, 2, 2)?;
    let Value::Object(object) = &args[0] else {
        return Err(type_error(format!(
            "Object.setPrototype expects an object, got {}",
            args[0].type_name()
        )));
    };
    let prototype = match &args[1] {
        Value::Null => None,
        Value::Object(_) => Some(args[1].clone()),
        value => {
            return Err(type_error(format!(
                "Object.setPrototype expects an object or null prototype, got {}",
                value.type_name()
            )));
        }
    };
    // Reject prototype cycles before mutating.
    let mut seen = vec![Arc::as_ptr(object)];
    let mut current = prototype.clone();
    while let Some(Value::Object(entry)) = current {
        let pointer = Arc::as_ptr(&entry);
        if seen.contains(&pointer) {
            return Err(type_error("prototype chains must not contain cycles"));
        }
        seen.push(pointer);
        current = entry.prototype();
    }
    object.set_prototype(prototype);
    Ok(Value::Null)
}

fn object_seal(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let object = object_arg("Object.seal", &args, 1, 1)?;
    object.set_sealed(true);
    Ok(args[0].clone())
}

fn object_is_sealed(_engine: &mut Engine, args: Vec<Value>) -> EvalResult<Value> {
    let object = object_arg("Object.isSealed", &args, 1, 1)?;
    Ok(Value::Bool(object.sealed()))
}
