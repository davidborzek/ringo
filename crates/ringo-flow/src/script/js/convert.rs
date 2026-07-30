//! Value marshalling between QuickJS (`rquickjs::Value`) and the engine's neutral
//! [`crate::engine::assertion::Value`], plus error → JS-exception conversion.
//!
//! This is the heart of the embedding: every getter result and every `expect(x)`
//! argument crosses this boundary. It is deliberately small — the spike only needs
//! bool / int / string / the `State` enum.

use crate::engine::assertion::Value as EngVal;
use crate::engine::ctx::CallState;
use rquickjs::{Array, Ctx, Error as JsError, Object, Result as JsResult, Value};

/// A parsed JSON value → a native JS value (for `response.json(...)`): objects →
/// objects, arrays → arrays, numbers → int/float, `null` → `undefined`.
pub fn json_to_js<'js>(ctx: &Ctx<'js>, v: &serde_json::Value) -> JsResult<Value<'js>> {
    use serde_json::Value as J;
    Ok(match v {
        J::Null => Value::new_undefined(ctx.clone()),
        J::Bool(b) => Value::new_bool(ctx.clone(), *b),
        J::Number(n) => match n.as_i64() {
            Some(i) => match i32::try_from(i) {
                Ok(v) => Value::new_int(ctx.clone(), v),
                // Beyond i32: JS numbers are f64, so widen instead of truncating.
                Err(_) => Value::new_float(ctx.clone(), i as f64),
            },
            None => Value::new_float(ctx.clone(), n.as_f64().unwrap_or(0.0)),
        },
        J::String(s) => rquickjs::String::from_str(ctx.clone(), s).map(Value::from_string)?,
        J::Array(a) => {
            let arr = Array::new(ctx.clone())?;
            for (i, it) in a.iter().enumerate() {
                arr.set(i, json_to_js(ctx, it)?)?;
            }
            arr.into_value()
        }
        J::Object(o) => {
            let obj = Object::new(ctx.clone())?;
            for (k, val) in o {
                obj.set(k.as_str(), json_to_js(ctx, val)?)?;
            }
            obj.into_value()
        }
    })
}

/// Turn an engine error string into a thrown JS exception (an `Error` whose
/// message is the engine text). Returning `Err(JsError::Exception)` after this
/// propagates it as a real JS throw, so the scenario fails cleanly.
pub fn throw(ctx: &Ctx<'_>, msg: &str) -> JsError {
    let s = rquickjs::String::from_str(ctx.clone(), msg)
        .map(Value::from_string)
        .unwrap_or_else(|_| Value::new_undefined(ctx.clone()));
    ctx.throw(s)
}

/// Engine value → JS value. The `State` enum is surfaced as its lowercase string
/// (matching the `.d.ts` `const enum State { Ringing = "ringing", … }`).
pub fn into_value<'js>(ctx: &Ctx<'js>, v: EngVal) -> JsResult<Value<'js>> {
    Ok(match v {
        EngVal::Unit => Value::new_undefined(ctx.clone()),
        EngVal::Bool(b) => Value::new_bool(ctx.clone(), b),
        EngVal::Int(i) => match i32::try_from(i) {
            Ok(v) => Value::new_int(ctx.clone(), v),
            Err(_) => Value::new_float(ctx.clone(), i as f64),
        },
        EngVal::Float(f) => Value::new_float(ctx.clone(), f),
        EngVal::Str(s) => rquickjs::String::from_str(ctx.clone(), &s).map(Value::from_string)?,
        EngVal::State(s) => {
            rquickjs::String::from_str(ctx.clone(), state_str(s)).map(Value::from_string)?
        }
        EngVal::List(items) => {
            let arr = rquickjs::Array::new(ctx.clone())?;
            for (i, it) in items.into_iter().enumerate() {
                arr.set(i, into_value(ctx, it)?)?;
            }
            arr.into_value()
        }
        EngVal::Map(pairs) => {
            let obj = rquickjs::Object::new(ctx.clone())?;
            for (k, v) in pairs {
                obj.set(k, into_value(ctx, v)?)?;
            }
            obj.into_value()
        }
    })
}

/// JS value → engine value (for `expect(x)` / `.equals(y)`). Whole numbers map to
/// `Int`, non-integers to `Float` (so `quality.mos` etc. keep their precision).
pub fn from_value(v: &Value<'_>) -> EngVal {
    if v.is_undefined() || v.is_null() {
        EngVal::Unit
    } else if let Some(b) = v.as_bool() {
        EngVal::Bool(b)
    } else if let Some(i) = v.as_int() {
        EngVal::Int(i as i64)
    } else if let Some(f) = v.as_float() {
        EngVal::Float(f)
    } else if let Some(s) = v.as_string() {
        EngVal::Str(s.to_string().unwrap_or_default())
    } else if let Some(arr) = v.as_array() {
        EngVal::List(
            arr.iter::<Value>()
                .filter_map(|r| r.ok())
                .map(|x| from_value(&x))
                .collect(),
        )
    } else if let Some(obj) = v.as_object() {
        EngVal::Map(
            obj.props::<String, Value>()
                .filter_map(|r| r.ok())
                .map(|(k, x)| (k, from_value(&x)))
                .collect(),
        )
    } else {
        // Fallback: stringify via the JS engine isn't available here without a Ctx;
        // an unknown type compares as its debug-ish unit (rare in the spike).
        EngVal::Unit
    }
}

/// The JS-side string for a call state (matches the `.d.ts` `State` enum values).
fn state_str(s: CallState) -> &'static str {
    match s {
        CallState::Idle => "idle",
        CallState::Ringing => "ringing",
        CallState::Established => "established",
    }
}
