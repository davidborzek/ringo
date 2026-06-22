//! Conversions between Rhai's dynamic values and the engine's neutral types: the
//! `Value` used by assertions, an `Account` from a config map, and custom-header
//! pairs. This is the only place Rhai value shapes are interpreted.

use crate::engine::assertion::Value;
use crate::engine::mock_server::MockResponse;
use crate::engine::{AgentInfo, CallState, sip_user_part};
use rhai::{Array, Dynamic, EvalAltResult, Map};
use ringo_core::baresip::Account;

/// A Rhai value → the engine's neutral [`Value`] (for assertions). `CallState`,
/// bool, int, unit, arrays and maps are recognised; everything else becomes a
/// string.
pub(super) fn to_value(d: &Dynamic) -> Value {
    if d.is_unit() {
        Value::Unit
    } else if let Some(c) = d.clone().try_cast::<CallState>() {
        Value::State(c)
    } else if let Ok(b) = d.as_bool() {
        Value::Bool(b)
    } else if let Ok(i) = d.as_int() {
        Value::Int(i)
    } else if let Some(arr) = d.clone().try_cast::<Array>() {
        Value::List(arr.iter().map(to_value).collect())
    } else if let Some(map) = d.clone().try_cast::<Map>() {
        Value::Map(
            map.iter()
                .map(|(k, v)| (k.to_string(), to_value(v)))
                .collect(),
        )
    } else if let Ok(s) = d.clone().into_string() {
        Value::Str(s)
    } else {
        Value::Str(d.to_string())
    }
}

/// The engine's neutral [`Value`] → a Rhai value (for `.value()`).
pub(super) fn to_dynamic(v: &Value) -> Dynamic {
    match v {
        Value::Unit => Dynamic::UNIT,
        Value::Bool(b) => (*b).into(),
        Value::Int(i) => (*i).into(),
        Value::Str(s) => s.clone().into(),
        Value::State(c) => Dynamic::from(*c),
        Value::List(items) => Dynamic::from_array(items.iter().map(to_dynamic).collect()),
        Value::Map(pairs) => {
            let mut m = Map::new();
            for (k, v) in pairs {
                m.insert(k.as_str().into(), to_dynamic(v));
            }
            Dynamic::from_map(m)
        }
    }
}

/// A parsed JSON value → a native Rhai value (for `response.json(...)`): objects
/// become maps, arrays become arrays, numbers int/float, `null` becomes `()`.
/// So a scenario can do `res.json("data").id` or `assert(res.json("count")).equals(3)`
/// without string-juggling.
pub(super) fn json_to_dynamic(v: &serde_json::Value) -> Dynamic {
    use serde_json::Value as J;
    match v {
        J::Null => Dynamic::UNIT,
        J::Bool(b) => (*b).into(),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into()
            } else if let Some(f) = n.as_f64() {
                f.into()
            } else {
                // u64 > i64::MAX: keep it exact as a string rather than lose it.
                n.to_string().into()
            }
        }
        J::String(s) => s.clone().into(),
        J::Array(a) => {
            let arr: Array = a.iter().map(json_to_dynamic).collect();
            Dynamic::from_array(arr)
        }
        J::Object(o) => {
            let mut m = Map::new();
            for (k, val) in o {
                m.insert(k.as_str().into(), json_to_dynamic(val));
            }
            Dynamic::from_map(m)
        }
    }
}

/// A Rhai value → `serde_json::Value` (for `json_response(...)`): maps become
/// objects, arrays become arrays, bool/int/float/string map directly, `()` becomes
/// `null`, and anything else is stringified. Recursive, so nested action lists
/// serialize as real JSON.
pub(super) fn dynamic_to_json(d: &Dynamic) -> serde_json::Value {
    use serde_json::Value as J;
    if d.is_unit() {
        J::Null
    } else if let Ok(b) = d.as_bool() {
        J::Bool(b)
    } else if let Ok(i) = d.as_int() {
        J::Number(i.into())
    } else if let Ok(f) = d.as_float() {
        serde_json::Number::from_f64(f).map_or(J::Null, J::Number)
    } else if let Some(arr) = d.clone().try_cast::<Array>() {
        J::Array(arr.iter().map(dynamic_to_json).collect())
    } else if let Some(map) = d.clone().try_cast::<Map>() {
        J::Object(
            map.iter()
                .map(|(k, v)| (k.to_string(), dynamic_to_json(v)))
                .collect(),
        )
    } else if let Ok(s) = d.clone().into_string() {
        J::String(s)
    } else {
        J::String(d.to_string())
    }
}

/// An [`AgentInfo`] snapshot → a Rhai map (for `agent.info()`). `state` is a
/// string so the map prints and `to_json()`s cleanly (no custom type inside).
pub(super) fn info_to_map(i: &AgentInfo) -> Dynamic {
    let mut m = Map::new();
    m.insert("name".into(), i.name.clone().into());
    m.insert("aor".into(), i.aor.clone().into());
    m.insert("registered".into(), i.registered.into());
    m.insert("state".into(), i.state.to_string().into());
    m.insert("reason".into(), opt_to_dynamic(i.reason.clone()));
    m.insert(
        "status_code".into(),
        match i.status_code {
            Some(c) => (c as i64).into(),
            None => Dynamic::UNIT,
        },
    );
    m.insert("peer".into(), peer_to_map(i.peer.clone()));
    m.insert("calls".into(), (i.calls as i64).into());
    Dynamic::from_map(m)
}

/// A remote party `(uri, display_name)` → a Rhai map `#{ uri, number, name }`
/// (`number` is the user-part of the URI, `name` is `()` if absent), or `()` if
/// there is no call. Used for the `peer` sub-object of `info()`/`to_json()`.
pub(super) fn peer_to_map(p: Option<(String, Option<String>)>) -> Dynamic {
    match p {
        Some((uri, name)) => {
            let mut m = Map::new();
            m.insert("number".into(), sip_user_part(&uri).into());
            m.insert("uri".into(), uri.into());
            m.insert("name".into(), opt_to_dynamic(name));
            Dynamic::from_map(m)
        }
        None => Dynamic::UNIT,
    }
}

/// Received INVITE headers `(name, value)` → a Rhai map (name → value). Duplicate
/// names collapse to the last value; use `header(name)` for a specific one.
pub(super) fn headers_to_map(headers: Vec<(String, String)>) -> Dynamic {
    let mut m = Map::new();
    for (k, v) in headers {
        m.insert(k.as_str().into(), v.into());
    }
    Dynamic::from_map(m)
}

/// An optional string value → Rhai (`None` becomes `()`), for getters like
/// `reason`/`header`/response `header`.
pub(super) fn opt_to_dynamic(v: Option<String>) -> Dynamic {
    match v {
        Some(s) => s.into(),
        None => Dynamic::UNIT,
    }
}

/// Build an [`Account`] from a Rhai config map (`#{ username: …, domain: … }`).
pub(super) fn account_from_map(name: &str, map: &Map) -> Result<Account, Box<EvalAltResult>> {
    let get = |k: &str| map.get(k).and_then(|d| d.clone().into_string().ok());
    let req = |k: &str| get(k).ok_or_else(|| format!("agent `{name}`: `{k}` is required"));
    Ok(Account {
        username: req("username")?,
        domain: req("domain")?,
        password: get("password").unwrap_or_default(),
        display_name: get("display_name"),
        transport: get("transport"),
        auth_user: get("auth_user"),
        outbound: get("outbound"),
        stun_server: get("stun_server"),
        media_enc: get("media_enc"),
        regint: map
            .get("regint")
            .and_then(|d| d.as_int().ok())
            .map(|i| i as u32),
        mwi: map
            .get("mwi")
            .and_then(|d| d.as_bool().ok())
            .unwrap_or(false),
        dtmf_mode: get("dtmf_mode"),
    })
}

/// `headers: #{ "X-Foo": "bar" }` → ordered (key, value) pairs. Header names are
/// validated as SIP tokens so they can't malform the `uaaddheader` command (no
/// CRLF, space, `:` etc.).
pub(super) fn headers_from_map(map: &Map) -> Result<Vec<(String, String)>, Box<EvalAltResult>> {
    let Some(h) = map.get("headers").and_then(|d| d.clone().try_cast::<Map>()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (k, v) in h.iter() {
        if !is_header_token(k) {
            return Err(format!("`{k}` is not a valid SIP header name").into());
        }
        if let Ok(val) = v.clone().into_string() {
            out.push((k.to_string(), val));
        }
    }
    Ok(out)
}

/// A mock-server response map → [`MockResponse`]. Shape:
/// `#{ status: 200, content_type: "…", headers: #{…}, body: <string|map> }`.
/// `status` defaults to 200; `body` defaults to empty; a map `body` is JSON-encoded
/// (so `json_response(...)` and a hand-written map both work).
pub(super) fn map_to_response(map: &Map) -> Result<MockResponse, Box<EvalAltResult>> {
    let status = match map.get("status") {
        Some(d) => d
            .as_int()
            .map_err(|_| -> Box<EvalAltResult> { "`status` must be an integer".into() })?,
        None => 200,
    };
    let status = u16::try_from(status)
        .map_err(|_| -> Box<EvalAltResult> { format!("`status` {status} out of range").into() })?;
    let content_type = map
        .get("content_type")
        .and_then(|d| d.clone().into_string().ok());
    let headers = match map.get("headers").and_then(|d| d.clone().try_cast::<Map>()) {
        Some(h) => h
            .iter()
            .filter_map(|(k, v)| v.clone().into_string().ok().map(|val| (k.to_string(), val)))
            .collect(),
        None => Vec::new(),
    };
    let body = map.get("body").and_then(body_to_string).unwrap_or_default();
    Ok(MockResponse {
        status,
        content_type,
        headers,
        body,
    })
}

/// A valid SIP header field name (RFC 3261 `token`).
fn is_header_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-.!%*_+`'~".contains(&b))
}

/// A `body` value → request body string: a map is JSON-encoded, anything else is
/// taken as a string (so `body: #{ a: 1 }` and `body: "…"` both work).
pub(super) fn body_to_string(d: &Dynamic) -> Option<String> {
    match d.clone().try_cast::<Map>() {
        Some(map) => Some(rhai::format_map_as_json(&map)),
        None => d.clone().into_string().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::{account_from_map, body_to_string, headers_from_map, json_to_dynamic};
    use rhai::{Dynamic, Map};

    #[test]
    fn json_to_dynamic_maps_types() {
        use serde_json::json;
        assert!(json_to_dynamic(&json!(null)).is_unit());
        assert_eq!(json_to_dynamic(&json!(true)).as_bool(), Ok(true));
        assert_eq!(json_to_dynamic(&json!(42)).as_int(), Ok(42));
        assert_eq!(
            json_to_dynamic(&json!("hi")).into_string(),
            Ok("hi".to_string())
        );
        // An object becomes a navigable map; an array a navigable array.
        let m = json_to_dynamic(&json!({"id": 7}))
            .try_cast::<Map>()
            .unwrap();
        assert_eq!(m.get("id").unwrap().as_int(), Ok(7));
        let a = json_to_dynamic(&json!(["a", "b"]))
            .try_cast::<rhai::Array>()
            .unwrap();
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn account_required_and_optional_fields() {
        let mut m = Map::new();
        m.insert("username".into(), Dynamic::from("alice"));
        m.insert("domain".into(), Dynamic::from("example.com"));
        m.insert("stun_server".into(), Dynamic::from("stun:x"));
        let acc = account_from_map("A", &m).unwrap();
        assert_eq!(acc.username, "alice");
        assert_eq!(acc.domain, "example.com");
        assert_eq!(acc.stun_server.as_deref(), Some("stun:x"));
        assert_eq!(acc.password, ""); // optional, defaults empty

        let mut bad = Map::new();
        bad.insert("username".into(), Dynamic::from("alice"));
        assert!(account_from_map("A", &bad).is_err());
    }

    #[test]
    fn headers_collected_from_submap() {
        let mut hdrs = Map::new();
        hdrs.insert("X-Foo".into(), Dynamic::from("bar"));
        let mut m = Map::new();
        m.insert("headers".into(), Dynamic::from(hdrs));
        assert_eq!(
            headers_from_map(&m).unwrap(),
            vec![("X-Foo".to_string(), "bar".to_string())]
        );
        assert!(headers_from_map(&Map::new()).unwrap().is_empty());

        let mut bad_hdrs = Map::new();
        bad_hdrs.insert("X-Bad\r\nInjected".into(), Dynamic::from("v"));
        let mut bad = Map::new();
        bad.insert("headers".into(), Dynamic::from(bad_hdrs));
        assert!(headers_from_map(&bad).is_err());
    }

    #[test]
    fn body_accepts_string_or_map() {
        assert_eq!(
            body_to_string(&Dynamic::from("raw")).as_deref(),
            Some("raw")
        );
        let mut m = Map::new();
        m.insert("announcement".into(), Dynamic::from(false));
        assert_eq!(
            body_to_string(&Dynamic::from(m)).as_deref(),
            Some(r#"{"announcement":false}"#)
        );
    }
}
