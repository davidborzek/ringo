//! The `Assertion` fluent JS class wrapping the neutral engine assertion, plus the
//! `expect(...)` global.

use super::super::convert::{from_value, into_value, throw};
use super::core::HostState;
use crate::engine::assertion::{Assertion as EngineAssertion, Value as EngVal};
use crate::engine::ctx::Ctx as EngineCtx;
use rquickjs::class::Trace;
use rquickjs::{Class, Ctx as JsCtx, JsLifetime, Result as JsResult, Value};
use std::sync::Arc;

/// A fluent assertion handle wrapping the neutral engine assertion. Matchers chain
/// by returning the handle; a failed matcher throws a JS exception (so the
/// scenario fails) while still reporting the `expected … but was …` line.
#[derive(Trace, JsLifetime, Clone)]
#[rquickjs::class(rename = "Assertion")]
pub struct Assertion {
    #[qjs(skip_trace)]
    inner: EngineAssertion,
}

impl Assertion {
    pub fn make(ctx: Arc<EngineCtx>, actual: EngVal) -> Self {
        Self {
            inner: EngineAssertion::new(ctx, actual),
        }
    }
}

#[ringo_flow_macros::ts_export(generic = "<T>")]
#[rquickjs::methods]
impl Assertion {
    /// Negate the next matcher (Jest-style): `expect(x).not.toBe(2)`. Applies only to
    /// the matcher immediately after — the handle it returns is positive again.
    #[qjs(get)]
    fn not<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Class<'js, Assertion>> {
        let mut inner = self.inner.clone();
        inner.set_negated(true);
        Class::instance(ctx.clone(), Assertion { inner })
    }

    /// A matcher result → a chainable `Assertion` handle, or a thrown JS exception on a
    /// failed check. The returned handle is positive again — `.not` negates only the one
    /// matcher after it — so chaining continues
    /// (`expect(x).toBeGreaterThanOrEqual(1).toBeLessThanOrEqual(9)`).
    #[qjs(skip)]
    fn chain<'js>(
        &self,
        ctx: &JsCtx<'js>,
        r: Result<(), String>,
    ) -> JsResult<Class<'js, Assertion>> {
        match r {
            Ok(()) => {
                let mut inner = self.inner.clone();
                inner.set_negated(false);
                Class::instance(ctx.clone(), Assertion { inner })
            }
            Err(e) => Err(throw(ctx, &e)),
        }
    }

    /// The value under assertion, so a verified value can be bound, e.g.
    /// `const id = await until(() => expect(callee.header("X-Id")).toBeDefined().value())`.
    #[jsdoc(type = "T")]
    fn value<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Value<'js>> {
        into_value(&ctx, self.inner.value().clone())
    }

    #[qjs(rename = "toBe")]
    fn equals<'js>(
        &self,
        ctx: JsCtx<'js>,
        #[jsdoc(type = "T")] expected: Value<'js>,
    ) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.equals(&from_value(&expected));
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeTruthy")]
    fn is_true<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.is_true();
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeFalsy")]
    fn is_false<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.is_false();
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeDefined")]
    fn is_present<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.is_present();
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeUndefined")]
    fn is_absent<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.is_absent();
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeEmpty")]
    fn is_empty<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.is_empty();
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toContain")]
    fn contains<'js>(&self, ctx: JsCtx<'js>, needle: String) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.contains(&needle);
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toMatch")]
    fn matches<'js>(&self, ctx: JsCtx<'js>, pattern: String) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.matches(&pattern);
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeGreaterThan")]
    fn greater_than<'js>(&self, ctx: JsCtx<'js>, n: f64) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.greater_than(n);
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeLessThan")]
    fn less_than<'js>(&self, ctx: JsCtx<'js>, n: f64) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.less_than(n);
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeGreaterThanOrEqual")]
    fn at_least<'js>(&self, ctx: JsCtx<'js>, n: f64) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.at_least(n);
        self.chain(&ctx, r)
    }

    #[qjs(rename = "toBeLessThanOrEqual")]
    fn at_most<'js>(&self, ctx: JsCtx<'js>, n: f64) -> JsResult<Class<'js, Assertion>> {
        let r = self.inner.at_most(n);
        self.chain(&ctx, r)
    }

    /// Label this assertion (`.as("caller registered")`) — chainable, Jest has no
    /// equivalent so the name avoids colliding with a test-grouping `describe`.
    #[qjs(rename = "as")]
    fn describe<'js>(&self, ctx: JsCtx<'js>, label: String) -> JsResult<Class<'js, Assertion>> {
        let mut inner = self.inner.clone();
        inner.describe(&label);
        Class::instance(ctx.clone(), Assertion { inner })
    }
}

/// Begin a fluent assertion on a value.
#[ringo_flow_macros::ts_global(name = "expect", generic = "<T>")]
#[jsdoc(type = "Assertion<T>")]
pub(in crate::script::js) fn expect_global<'js>(
    cx: JsCtx<'js>,
    #[jsdoc(type = "T")] actual: Value<'js>,
) -> rquickjs::Result<Class<'js, Assertion>> {
    let eng = cx
        .userdata::<HostState>()
        .expect("host state stored at install")
        .eng
        .clone();
    Class::instance(cx, Assertion::make(eng, from_value(&actual)))
}
