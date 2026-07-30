//! The `HttpResponse` JS class and the async `http(...)` global plus its options
//! parsing.

use super::super::convert::{into_value, throw};
use super::core::HostState;
use crate::engine::assertion::Value as EngVal;
use indexmap::IndexMap;
use rquickjs::class::Trace;
use rquickjs::function::Opt;
use rquickjs::{Class, Ctx as JsCtx, JsLifetime, Object, Result as JsResult, Value};

/// The `http(method, url, { … })` options. A shape-only marker mirroring the
/// `HttpOptions` interface in the hand-written `.d.ts`; the fields are never read off
/// it (parsing stays hand-written in [`http_options`] for the string-or-object body),
/// only [`HttpOptions::FIELDS`] is used, for unknown-key rejection.
#[allow(dead_code)]
#[derive(ringo_flow_macros::TsInterface)]
struct HttpOptions {
    /// Request headers to send.
    headers: Option<IndexMap<String, String>>,
    /// request body; an object is JSON-encoded.
    #[jsdoc(type = "string | object", optional)]
    body: Option<String>,
}

/// The response of an `http(method, url)` call. Thin wrapper over the engine's
/// `HttpResponse`; `json(path)` returns a native JS value.
#[derive(Trace, JsLifetime, Clone)]
#[rquickjs::class(rename = "HttpResponse")]
pub struct HttpResponse {
    #[qjs(skip_trace)]
    pub inner: crate::engine::http::HttpResponse,
}

#[ringo_flow_macros::ts_export]
#[rquickjs::methods]
impl HttpResponse {
    #[qjs(get)]
    fn status(&self) -> i64 {
        self.inner.status()
    }
    #[qjs(get)]
    fn body(&self) -> String {
        self.inner.body()
    }
    #[jsdoc(type = "string | undefined")]
    fn header<'js>(&self, ctx: JsCtx<'js>, name: String) -> JsResult<Value<'js>> {
        into_value(
            &ctx,
            self.inner.header(&name).map_or(EngVal::Unit, EngVal::Str),
        )
    }
    /// The JSON value at a dotted `path` (empty for the whole body), as a native
    /// JS value.
    fn json<'js>(&self, ctx: JsCtx<'js>, path: Opt<String>) -> JsResult<Value<'js>> {
        match self.inner.json(&path.0.unwrap_or_default()) {
            Ok(v) => super::super::convert::json_to_js(&ctx, &v),
            Err(e) => Err(throw(&ctx, &e)),
        }
    }
    #[qjs(rename = "expectStatus")]
    fn expect_status<'js>(&self, ctx: JsCtx<'js>, code: i64) -> JsResult<()> {
        self.inner.expect_status(code).map_err(|e| throw(&ctx, &e))
    }
}

/// Extract `{ headers, body }` from the options object on the JS thread (the
/// `Object` is `!Send`, so this can't move into `spawn_blocking`). `body` may be a
/// string (sent as-is) or an object (JSON-encoded). Returns owned, `Send` values.
fn http_options<'js>(
    cx: &JsCtx<'js>,
    opts: &Object<'js>,
) -> Result<(Vec<(String, String)>, Option<String>), String> {
    super::super::bindings::reject_unknown_keys("http options", opts, HttpOptions::FIELDS)?;
    let headers = match opts
        .get::<_, Option<Object>>("headers")
        .map_err(|_| "http options: `headers` must be an object".to_string())?
    {
        Some(h) => h
            .props::<String, String>()
            .collect::<rquickjs::Result<Vec<_>>>()
            .map_err(|_| "http options: header values must be strings".to_string())?,
        None => Vec::new(),
    };
    let body: Value = opts
        .get("body")
        .map_err(|_| "http options: `body` unreadable".to_string())?;
    let body = if body.is_undefined() || body.is_null() {
        None
    } else if let Some(s) = body.as_string() {
        Some(
            s.to_string()
                .map_err(|_| "http options: bad `body` string".to_string())?,
        )
    } else if body.is_object() {
        // A map/object body is encoded to JSON (like rhai).
        cx.json_stringify(body)
            .ok()
            .flatten()
            .map(|s| s.to_string().unwrap_or_default())
            .or(Some(String::new()))
    } else {
        return Err("http options: `body` must be a string or an object".to_string());
    };
    Ok((headers, body))
}

/// Performs the request off-thread and resolves with the response; `await` it.
/// `await Promise.all([http(...), http(...)])` fires several requests concurrently.
#[ringo_flow_macros::ts_global(name = "http")]
pub(in crate::script::js) async fn http_async<'js>(
    cx: JsCtx<'js>,
    method: String,
    url: String,
    #[jsdoc(type = "HttpOptions")] opts: Opt<Object<'js>>,
) -> rquickjs::Result<Class<'js, HttpResponse>> {
    let eng = cx
        .userdata::<HostState>()
        .expect("host state stored at install")
        .eng
        .clone();
    let (headers, body) = match &opts.0 {
        Some(o) => http_options(&cx, o).map_err(|e| throw(&cx, &e))?,
        None => (Vec::new(), None),
    };
    let handle = eng.rt.clone();
    match handle
        .spawn_blocking(move || crate::engine::http::perform(&eng, &method, &url, &headers, body))
        .await
    {
        Ok(Ok(inner)) => Class::instance(cx, HttpResponse { inner }),
        Ok(Err(e)) => Err(throw(&cx, &e)),
        Err(e) => Err(throw(&cx, &format!("http task failed: {e}"))),
    }
}
