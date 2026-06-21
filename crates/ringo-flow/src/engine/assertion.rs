//! Language-neutral fluent assertions (AssertJ-style) + `await_until`
//! (Awaitility-style). A language adapter converts its native values into
//! [`Value`], drives [`Assertion`] matchers, and wraps a script closure into the
//! `body` of [`await_until`].

use super::ctx::{CallState, Ctx};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A value under assertion, across the types scenarios compare.
#[derive(Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Str(String),
    State(CallState),
    /// An array (e.g. `res.json("items")`).
    List(Vec<Value>),
    /// An object map (e.g. `agent.headers()`), key → value.
    Map(Vec<(String, Value)>),
}

impl Value {
    /// Display string (used in reports and equality).
    pub fn display(&self) -> String {
        match self {
            Value::Unit => "()".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Str(s) => s.clone(),
            Value::State(s) => s.to_string(),
            Value::List(items) => {
                let body = items
                    .iter()
                    .map(Value::display)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{body}]")
            }
            Value::Map(pairs) => {
                let body = pairs
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.display()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("#{{{body}}}")
            }
        }
    }
    /// A coarse type tag so equality never matches across types (`1` vs `"1"`).
    fn type_tag(&self) -> u8 {
        match self {
            Value::Unit => 0,
            Value::Bool(_) => 1,
            Value::Int(_) => 2,
            Value::Str(_) => 3,
            Value::State(_) => 4,
            Value::List(_) => 5,
            Value::Map(_) => 6,
        }
    }
    fn as_int(&self) -> Result<i64, String> {
        match self {
            Value::Int(i) => Ok(*i),
            other => Err(format!(
                "expected a number to compare, but was {}",
                other.display()
            )),
        }
    }
    /// Whether this is an empty string/array/map; `Err` for scalars that have no
    /// notion of emptiness.
    fn is_empty_collection(&self) -> Result<bool, String> {
        match self {
            Value::Str(s) => Ok(s.is_empty()),
            Value::List(items) => Ok(items.is_empty()),
            Value::Map(pairs) => Ok(pairs.is_empty()),
            other => Err(format!(
                "expected a string, array or map to check emptiness, but was {}",
                other.display()
            )),
        }
    }
}

fn value_eq(a: &Value, b: &Value) -> bool {
    a.type_tag() == b.type_tag() && a.display() == b.display()
}

/// A value under assertion plus the context to report through.
#[derive(Clone)]
pub struct Assertion {
    actual: Value,
    /// Optional label set via `.describe(...)`, prefixed to the log line.
    desc: Option<String>,
    ctx: Arc<Ctx>,
}

impl Assertion {
    pub fn new(ctx: Arc<Ctx>, actual: Value) -> Self {
        Self {
            actual,
            // Auto-label from the getter that produced `actual`, if any
            // (`assert(caller.state)` → "Caller state"); `describe(...)` overrides.
            desc: super::ctx::take_pending_label(),
            ctx,
        }
    }
    /// The value under assertion (so an adapter can expose `.value()`).
    pub fn value(&self) -> &Value {
        &self.actual
    }
    /// Attach a label to this assertion (chainable upstream).
    pub fn describe(&mut self, label: &str) {
        self.desc = Some(label.to_string());
    }

    /// Report `expect` vs the actual value and turn a failure into an error.
    fn finish(&self, expect: String, pass: bool) -> Result<(), String> {
        let actual = self.actual.display();
        self.ctx
            .report_assertion(self.desc.clone(), expect.clone(), actual.clone(), pass);
        if pass {
            Ok(())
        } else {
            let label = self
                .desc
                .as_deref()
                .map(|d| format!("{d}: "))
                .unwrap_or_default();
            Err(format!("{label}expected {expect}, but was {actual}"))
        }
    }

    pub fn equals(&self, expected: &Value) -> Result<(), String> {
        let pass = value_eq(&self.actual, expected);
        self.finish(format!("equals {}", expected.display()), pass)
    }
    pub fn not_equals(&self, expected: &Value) -> Result<(), String> {
        let pass = !value_eq(&self.actual, expected);
        self.finish(format!("not equals {}", expected.display()), pass)
    }
    pub fn is_true(&self) -> Result<(), String> {
        let pass = matches!(self.actual, Value::Bool(true));
        self.finish("is true".into(), pass)
    }
    pub fn is_false(&self) -> Result<(), String> {
        let pass = matches!(self.actual, Value::Bool(false));
        self.finish("is false".into(), pass)
    }
    pub fn is_present(&self) -> Result<(), String> {
        let pass = !matches!(self.actual, Value::Unit);
        self.finish("is present".into(), pass)
    }
    pub fn is_absent(&self) -> Result<(), String> {
        let pass = matches!(self.actual, Value::Unit);
        self.finish("is absent".into(), pass)
    }
    pub fn is_empty(&self) -> Result<(), String> {
        let pass = self.actual.is_empty_collection()?;
        self.finish("is empty".into(), pass)
    }
    pub fn is_not_empty(&self) -> Result<(), String> {
        let pass = !self.actual.is_empty_collection()?;
        self.finish("is not empty".into(), pass)
    }
    pub fn contains(&self, needle: &str) -> Result<(), String> {
        let pass = self.actual.display().contains(needle);
        self.finish(format!("contains {needle:?}"), pass)
    }
    pub fn matches(&self, pattern: &str) -> Result<(), String> {
        let re =
            regex::Regex::new(pattern).map_err(|e| format!("invalid regex {pattern:?}: {e}"))?;
        let pass = re.is_match(&self.actual.display());
        self.finish(format!("matches {pattern:?}"), pass)
    }
    pub fn greater_than(&self, n: i64) -> Result<(), String> {
        let pass = self.actual.as_int()? > n;
        self.finish(format!("greater than {n}"), pass)
    }
    pub fn at_least(&self, n: i64) -> Result<(), String> {
        let pass = self.actual.as_int()? >= n;
        self.finish(format!("at least {n}"), pass)
    }
    pub fn less_than(&self, n: i64) -> Result<(), String> {
        let pass = self.actual.as_int()? < n;
        self.finish(format!("less than {n}"), pass)
    }
    pub fn at_most(&self, n: i64) -> Result<(), String> {
        let pass = self.actual.as_int()? <= n;
        self.finish(format!("at most {n}"), pass)
    }
}

/// Re-run `body` until it succeeds or `timeout` elapses. Assertions are silenced
/// while polling; the last one is emitted once when it settles. `body` is the
/// adapter's bridge to a script closure (returning `Ok` once the assertion holds).
pub fn await_until<F>(ctx: &Arc<Ctx>, mut body: F, timeout: Duration) -> Result<(), String>
where
    F: FnMut() -> Result<(), String>,
{
    ctx.set_assert_silent(true);
    let deadline = Instant::now() + timeout;
    let outcome = loop {
        match body() {
            Ok(()) => break Ok(()),
            Err(e) => {
                if Instant::now() >= deadline {
                    break Err(e);
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    };
    ctx.set_assert_silent(false);
    ctx.emit_last_assert();
    outcome.map_err(|e| format!("not satisfied within {timeout:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::Value;

    #[test]
    fn emptiness_and_display_for_collections() {
        assert!(Value::List(vec![]).is_empty_collection().unwrap());
        assert!(
            !Value::List(vec![Value::Int(1)])
                .is_empty_collection()
                .unwrap()
        );
        assert!(Value::Map(vec![]).is_empty_collection().unwrap());
        assert!(Value::Str(String::new()).is_empty_collection().unwrap());
        // scalars have no notion of emptiness
        assert!(Value::Int(1).is_empty_collection().is_err());

        assert_eq!(
            Value::List(vec![Value::Int(1), Value::Str("a".into())]).display(),
            "[1, a]"
        );
        assert_eq!(
            Value::Map(vec![("k".into(), Value::Int(2))]).display(),
            "#{k: 2}"
        );
    }
}
