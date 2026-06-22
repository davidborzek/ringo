//! Language-neutral HTTP verb: perform a request (blocking) and expose the
//! response plus a dotted-JSON-path accessor. Adapters wrap [`HttpResponse`] with
//! their own getters.

use super::ctx::Ctx;
use crate::runtime::report::Event;
use anyhow::{Context, bail};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// An HTTP response: status, lower-cased headers, body, plus the context to
/// report `expect_status` through.
#[derive(Clone)]
pub struct HttpResponse {
    pub status: i64,
    pub headers: HashMap<String, String>, // lower-cased names
    pub body: String,
    ctx: Arc<Ctx>,
}

impl HttpResponse {
    pub fn status(&self) -> i64 {
        super::ctx::mark_pending_label("HTTP status");
        self.status
    }
    pub fn body(&self) -> String {
        super::ctx::mark_pending_label("HTTP body");
        self.body.clone()
    }
    /// A response header value, or `None` if absent.
    pub fn header(&self, name: &str) -> Option<String> {
        super::ctx::mark_pending_label(format!("HTTP header {name}"));
        self.headers.get(&name.to_lowercase()).cloned()
    }
    /// The JSON value at a dotted path in the body, or the whole body for an empty
    /// path. An empty/absent body is reported as JSON `null`. Returned as a neutral
    /// `serde_json::Value`; the language adapter turns it into a native value
    /// (map/array/number/bool/null).
    pub fn json(&self, path: &str) -> Result<serde_json::Value, String> {
        super::ctx::mark_pending_label(if path.is_empty() {
            "HTTP body (JSON)".to_string()
        } else {
            format!("HTTP {path}")
        });
        json_path_value(&self.body, path).map_err(|e| e.to_string())
    }
    /// Assert and report the status; errors on mismatch.
    pub fn expect_status(&self, code: i64) -> Result<(), String> {
        let ok = self.status == code;
        self.ctx.emit(&Event::Assertion {
            label: None,
            expect: format!("status is {code}"),
            ok,
            actual: Some(format!("status {}", self.status)),
        });
        if ok {
            Ok(())
        } else {
            Err(format!("expected status {code}, got {}", self.status))
        }
    }
}

/// Render an error with its full `source()` chain — reqwest's `Display` hides the
/// underlying cause (DNS / connect / TLS), which is exactly what you need to debug
/// a failed request.
fn error_chain(e: &dyn std::error::Error) -> String {
    let mut msg = e.to_string();
    let mut src = e.source();
    while let Some(s) = src {
        msg.push_str(&format!(": {s}"));
        src = s.source();
    }
    msg
}

/// Perform the request on the current (blocking) thread. `reqwest::blocking`
/// builds its own runtime, which is fine here — the whole script runs on a
/// `spawn_blocking` thread, not a runtime worker.
pub fn perform(
    ctx: &Arc<Ctx>,
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<String>,
) -> Result<HttpResponse, String> {
    let mut builder = reqwest::blocking::Client::builder().timeout(Duration::from_secs(30));
    if ctx.http_insecure() {
        // `--insecure-http`: skip TLS cert verification (e.g. internal services with
        // a private CA / self-signed cert). DANGER — opt-in only.
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder
        .build()
        .map_err(|e| format!("build HTTP client: {e}"))?;
    let m = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| format!("invalid HTTP method `{method}`"))?;
    let mut req = client.request(m, url);
    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }
    if let Some(b) = body {
        req = req.body(b);
    }
    let resp = req
        .send()
        .map_err(|e| format!("HTTP request to {url}: {}", error_chain(&e)))?;
    let status = resp.status().as_u16() as i64;
    let resp_headers = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let body = resp
        .text()
        .map_err(|e| format!("read HTTP response body: {e}"))?;

    ctx.emit(&Event::Http {
        method,
        url,
        status: status as u16,
    });
    Ok(HttpResponse {
        status,
        headers: resp_headers,
        body,
        ctx: ctx.clone(),
    })
}

/// Navigate a dotted path (`a.b.0.c`) into a JSON body and return the value at it
/// (the whole body for an empty path). Missing fields / out-of-range indices /
/// descending past a scalar are errors with the offending segment.
pub(crate) fn json_path_value(body: &str, path: &str) -> anyhow::Result<serde_json::Value> {
    // An empty body (e.g. 204 No Content) is treated as JSON `null` so `json()`
    // yields `()` rather than an error — `assert(body).is_present()` then guards
    // both the empty and the literal-`null` case uniformly. A non-empty body that
    // isn't valid JSON is still a real error.
    let root: serde_json::Value = if body.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(body).context("response body is not valid JSON")?
    };
    let mut cur = &root;
    for seg in path.split('.').filter(|s| !s.is_empty()) {
        cur = match cur {
            serde_json::Value::Object(map) => map
                .get(seg)
                .with_context(|| format!("no field `{seg}` in JSON path `{path}`"))?,
            serde_json::Value::Array(arr) => {
                let i: usize = seg
                    .parse()
                    .with_context(|| format!("`{seg}` is not an array index in `{path}`"))?;
                arr.get(i)
                    .with_context(|| format!("index {i} out of range in JSON path `{path}`"))?
            }
            _ => bail!("JSON path `{path}` descends past a scalar at `{seg}`"),
        };
    }
    Ok(cur.clone())
}

#[cfg(test)]
mod tests {
    use super::json_path_value;

    const BODY: &str = r#"{"state":"ringing","nested":{"id":42},"items":["a","b"]}"#;

    #[test]
    fn json_path_navigates_objects_and_arrays() {
        use serde_json::json;
        // Values keep their JSON type (string, number, …), not stringified.
        assert_eq!(json_path_value(BODY, "state").unwrap(), json!("ringing"));
        assert_eq!(json_path_value(BODY, "nested.id").unwrap(), json!(42));
        assert_eq!(json_path_value(BODY, "items.1").unwrap(), json!("b"));
        // An object path yields the sub-object; an empty path the whole body.
        assert_eq!(json_path_value(BODY, "nested").unwrap(), json!({"id": 42}));
        assert!(json_path_value(BODY, "").unwrap().is_object());
    }

    #[test]
    fn json_path_errors_are_descriptive() {
        assert!(json_path_value(BODY, "missing").is_err());
        assert!(json_path_value(BODY, "items.9").is_err());
        assert!(json_path_value(BODY, "state.x").is_err()); // past a scalar
        assert!(json_path_value("not json", "x").is_err()); // non-empty, not JSON
    }

    #[test]
    fn empty_body_is_json_null() {
        use serde_json::json;
        // An empty / whitespace-only body reads as `null` (→ `()` in scenarios),
        // so `assert(res.json()).is_present()` guards it instead of throwing.
        assert_eq!(json_path_value("", "").unwrap(), json!(null));
        assert_eq!(json_path_value("   \n", "").unwrap(), json!(null));
    }
}
