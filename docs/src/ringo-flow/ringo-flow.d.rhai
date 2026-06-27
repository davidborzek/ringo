module static;

op ==(int, int) -> bool;
op !=(int, int) -> bool;
op >(int, int) -> bool;
op >=(int, int) -> bool;
op <(int, int) -> bool;
op <=(int, int) -> bool;
op &(int, int) -> int;
op |(int, int) -> int;
op ^(int, int) -> int;
op ..(int, int) -> Range<int>;
op ..=(int, int) -> RangeInclusive<int>;

op ==(bool, bool) -> bool;
op !=(bool, bool) -> bool;
op >(bool, bool) -> bool;
op >=(bool, bool) -> bool;
op <(bool, bool) -> bool;
op <=(bool, bool) -> bool;
op &(bool, bool) -> bool;
op |(bool, bool) -> bool;
op ^(bool, bool) -> bool;

op ==((), ()) -> bool;
op !=((), ()) -> bool;
op >((), ()) -> bool;
op >=((), ()) -> bool;
op <((), ()) -> bool;
op <=((), ()) -> bool;

op +(int, int) -> int;
op -(int, int) -> int;
op *(int, int) -> int;
op /(int, int) -> int;
op %(int, int) -> int;
op **(int, int) -> int;
op >>(int, int) -> int;
op <<(int, int) -> int;

op +(float, float) -> float;
op -(float, float) -> float;
op *(float, float) -> float;
op /(float, float) -> float;
op %(float, float) -> float;
op **(float, float) -> float;
op ==(float, float) -> bool;
op !=(float, float) -> bool;
op >(float, float) -> bool;
op >=(float, float) -> bool;
op <(float, float) -> bool;
op <=(float, float) -> bool;

op +(float, int) -> float;
op -(float, int) -> float;
op *(float, int) -> float;
op /(float, int) -> float;
op %(float, int) -> float;
op **(float, int) -> float;
op ==(float, int) -> bool;
op !=(float, int) -> bool;
op >(float, int) -> bool;
op >=(float, int) -> bool;
op <(float, int) -> bool;
op <=(float, int) -> bool;

op +(int, float) -> float;
op -(int, float) -> float;
op *(int, float) -> float;
op /(int, float) -> float;
op %(int, float) -> float;
op **(int, float) -> float;
op ==(int, float) -> bool;
op !=(int, float) -> bool;
op >(int, float) -> bool;
op >=(int, float) -> bool;
op <(int, float) -> bool;
op <=(int, float) -> bool;

op +(Decimal, Decimal) -> Decimal;
op -(Decimal, Decimal) -> Decimal;
op *(Decimal, Decimal) -> Decimal;
op /(Decimal, Decimal) -> Decimal;
op %(Decimal, Decimal) -> Decimal;
op **(Decimal, Decimal) -> Decimal;
op ==(Decimal, Decimal) -> bool;
op !=(Decimal, Decimal) -> bool;
op >(Decimal, Decimal) -> bool;
op >=(Decimal, Decimal) -> bool;
op <(Decimal, Decimal) -> bool;
op <=(Decimal, Decimal) -> bool;

op +(Decimal, int) -> Decimal;
op -(Decimal, int) -> Decimal;
op *(Decimal, int) -> Decimal;
op /(Decimal, int) -> Decimal;
op %(Decimal, int) -> Decimal;
op **(Decimal, int) -> Decimal;
op ==(Decimal, int) -> bool;
op !=(Decimal, int) -> bool;
op >(Decimal, int) -> bool;
op >=(Decimal, int) -> bool;
op <(Decimal, int) -> bool;
op <=(Decimal, int) -> bool;

op +(int, Decimal) -> Decimal;
op -(int, Decimal) -> Decimal;
op *(int, Decimal) -> Decimal;
op /(int, Decimal) -> Decimal;
op %(int, Decimal) -> Decimal;
op **(int, Decimal) -> Decimal;
op ==(int, Decimal) -> bool;
op !=(int, Decimal) -> bool;
op >(int, Decimal) -> bool;
op >=(int, Decimal) -> bool;
op <(int, Decimal) -> bool;
op <=(int, Decimal) -> bool;

op +(String, String) -> String;
op -(String, String) -> String;
op ==(String, String) -> bool;
op !=(String, String) -> bool;
op >(String, String) -> bool;
op >=(String, String) -> bool;
op <(String, String) -> bool;
op <=(String, String) -> bool;

op +(char, char) -> String;
op ==(char, char) -> bool;
op !=(char, char) -> bool;
op >(char, char) -> bool;
op >=(char, char) -> bool;
op <(char, char) -> bool;
op <=(char, char) -> bool;

op +(char, String) -> String;
op ==(char, String) -> bool;
op !=(char, String) -> bool;
op >(char, String) -> bool;
op >=(char, String) -> bool;
op <(char, String) -> bool;
op <=(char, String) -> bool;

op +(String, char) -> String;
op -(String, char) -> String;
op ==(String, char) -> bool;
op !=(String, char) -> bool;
op >(String, char) -> bool;
op >=(String, char) -> bool;
op <(String, char) -> bool;
op <=(String, char) -> bool;

op +((), String) -> String;
op ==((), String) -> bool;
op !=((), String) -> bool;
op >((), String) -> bool;
op >=((), String) -> bool;
op <((), String) -> bool;
op <=((), String) -> bool;

op +(String, ()) -> String;
op ==(String, ()) -> bool;
op !=(String, ()) -> bool;
op >(String, ()) -> bool;
op >=(String, ()) -> bool;
op <(String, ()) -> bool;
op <=(String, ()) -> bool;

op +(Blob, Blob) -> Blob;
op +(Blob, char) -> Blob;
op ==(Blob, Blob) -> bool;
op !=(Blob, Blob) -> bool;


op ==(Range<int>, RangeInclusive<int>) -> bool;
op !=(Range<int>, RangeInclusive<int>) -> bool;

op ==(RangeInclusive<int>, Range<int>) -> bool;
op !=(RangeInclusive<int>, Range<int>) -> bool;

op ==(Range<int>, Range<int>) -> bool;
op !=(Range<int>, Range<int>) -> bool;

op ==(RangeInclusive<int>, RangeInclusive<int>) -> bool;
op !=(RangeInclusive<int>, RangeInclusive<int>) -> bool;

op ==(?, ?) -> bool;
op !=(?, ?) -> bool;
op >(?, ?) -> bool;
op >=(?, ?) -> bool;
op <(?, ?) -> bool;
op <=(?, ?) -> bool;


op &=(bool, bool);
op |=(bool, bool);

op +=(int, int);
op -=(int, int);
op *=(int, int);
op /=(int, int);
op %=(int, int);
op **=(int, int);
op >>=(int, int);
op <<=(int, int);
op &=(int, int);
op |=(int, int);
op ^=(int, int);

op +=(float, float);
op -=(float, float);
op *=(float, float);
op /=(float, float);
op %=(float, float);
op **=(float, float);

op +=(float, int);
op -=(float, int);
op *=(float, int);
op /=(float, int);
op %=(float, int);
op **=(float, int);

op +=(Decimal, Decimal);
op -=(Decimal, Decimal);
op *=(Decimal, Decimal);
op /=(Decimal, Decimal);
op %=(Decimal, Decimal);
op **=(Decimal, Decimal);

op +=(Decimal, int);
op -=(Decimal, int);
op *=(Decimal, int);
op /=(Decimal, int);
op %=(Decimal, int);
op **=(Decimal, int);

op +=(String, String);
op -=(String, String);
op +=(String, char);
op -=(String, char);
op +=(char, String);
op +=(char, char);

op +=(Array, Array);
op +=(Array, ?);

op +=(Blob, Blob);
op +=(Blob, int);
op +=(Blob, char);
op +=(Blob, String);

op in(?, Array) -> bool;
op in(String, String) -> bool;
op in(char, String) -> bool;
op in(int, Range<int>) -> bool;
op in(int, RangeInclusive<int>) -> bool;
op in(String, Map) -> bool;
op in(int, Blob) -> bool;

/// Display any data to the standard output.
///
/// # Example
/// 
/// ```rhai
/// let answer = 42;
/// 
/// print(`The Answer is ${answer}`);
/// ```
fn print(data: ?);

/// Display any data to the standard output in debug format.
///
/// # Example
/// 
/// ```rhai
/// let answer = 42;
/// 
/// debug(answer);
/// ```
fn debug(data: ?);

/// Get the type of a value.
///
/// # Example
/// 
/// ```rhai
/// let x = "hello, world!";
/// 
/// print(x.type_of());     // prints "string"
/// ```
fn type_of(data: ?) -> String;

/// Create a function pointer to a named function.
///
/// If the specified name is not a valid function name, an error is raised.
/// 
/// # Example
/// 
/// ```rhai
/// let f = Fn("foo");      // function pointer to 'foo'
/// 
/// f.call(42);             // call: foo(42)
/// ```
fn Fn(fn_name: String) -> FnPtr;

/// Call a function pointed to by a function pointer,
/// passing following arguments to the function call.
///
/// If an appropriate function is not found, an error is raised.
/// 
/// # Example
/// 
/// ```rhai
/// let f = Fn("foo");      // function pointer to 'foo'
/// 
/// f.call(1, 2, 3);        // call: foo(1, 2, 3)
/// ```
fn call(fn_ptr: FnPtr, ...args: ?) -> ?;

/// Call a function pointed to by a function pointer, binding the `this` pointer
/// to the object of the method call, and passing on following arguments to the function call.
///
/// If an appropriate function is not found, an error is raised.
/// 
/// # Example
/// 
/// ```rhai
/// fn add(x) {
///     this + x
/// }
/// 
/// let f = Fn("add");      // function pointer to 'add'
/// 
/// let x = 41;
/// 
/// let r = x.call(f, 1);   // call: add(1) with 'this' = 'x'
/// 
/// print(r);               // prints 42
/// ```
fn call(obj: ?, fn_ptr: FnPtr, ...args: ?) -> ?;

/// Curry a number of arguments into a function pointer and return it as a new function pointer.
/// 
/// # Example
/// 
/// ```rhai
/// fn foo(x, y, z) {
///     x + y + z
/// }
/// 
/// let f = Fn("foo");
/// 
/// let g = f.curry(1, 2);  // curried arguments: 1, 2
/// 
/// g.call(3);              // call: foo(1, 2, 3)
/// ```
fn curry(fn_ptr: FnPtr, ...args: ?) -> FnPtr;

/// Return `true` if a script-defined function exists with a specified name and
/// number of parameters.
/// 
/// # Example
/// 
/// ```rhai
/// fn foo(x) { }
/// 
/// print(is_def_fn("foo", 1));     // prints true
/// print(is_def_fn("foo", 2));     // prints false
/// print(is_def_fn("foo", 0));     // prints false
/// print(is_def_fn("bar", 1));     // prints false
/// ```
fn is_def_fn(fn_name: String, num_params: int) -> bool;

/// Return `true` if a script-defined function exists with a specified name and
/// number of parameters, bound to a specified type for `this`.
/// 
/// # Example
/// 
/// ```rhai
/// // A method that requires `this` to be `MyType`
/// fn MyType.foo(x) { }
/// 
/// print(is_def_fn("MyType", "foo", 1));   // prints true
/// print(is_def_fn("foo", 1));             // prints false
/// print(is_def_fn("MyType", "foo", 2));   // prints false
/// ```
fn is_def_fn(this_type: String, fn_name: String, num_params: int) -> bool;

/// Return `true` if a variable matching a specified name is defined.
/// 
/// # Example
/// 
/// ```rhai
/// let x = 42;
/// 
/// print(is_def_var("x"));         // prints true
/// print(is_def_var("foo"));       // prints false
/// 
/// {
///     let y = 1;
///     print(is_def_var("y"));     // prints true
/// }
/// 
/// print(is_def_var("y"));         // prints false
/// ```
fn is_def_var(var_name: String) -> bool;

/// Return `true` if the variable is shared.
/// 
/// # Example
/// 
/// ```rhai
/// let x = 42;
/// 
/// print(is_shared(x));        // prints false
/// 
/// let f = || x;               // capture 'x', making it shared
/// 
/// print(is_shared(x));        // prints true
/// ```
fn is_shared(variable: ?) -> bool;

/// Evaluate a text script within the current scope.
/// 
/// # Example
/// 
/// ```rhai
/// let x = 42;
/// 
/// eval("let y = x; x = 123;");
/// 
/// print(x);           // prints 123
/// print(y);           // prints 42
/// ```
fn eval(script: String) -> ?;

/// Return `true` if the string contains another string.
///
/// This function also drives the `in` operator.
///
/// # Example
///
/// ```rhai
/// let x = "hello world!";
///
/// // The 'in' operator calls 'contains' in the background
/// if "world" in x {
///     print("found!");
/// }
/// ```
fn contains(string: String, find: String) -> bool;

/// Return `true` if the string contains a character.
///
/// This function also drives the `in` operator.
///
/// # Example
///
/// ```rhai
/// let x = "hello world!";
///
/// // The 'in' operator calls 'contains' in the background
/// if 'w' in x {
///     print("found!");
/// }
/// ```
fn contains(string: String, ch: char) -> bool;

/// Return `true` if a value falls within the exclusive range.
///
/// This function also drives the `in` operator.
///
/// # Example
///
/// ```rhai
/// let r = 1..100;
///
/// // The 'in' operator calls 'contains' in the background
/// if 42 in r {
///     print("found!");
/// }
/// ```
fn contains(range: Range<int>, value: int) -> bool;

/// Return `true` if a value falls within the inclusive range.
///
/// This function also drives the `in` operator.
///
/// # Example
///
/// ```rhai
/// let r = 1..=100;
///
/// // The 'in' operator calls 'contains' in the background
/// if 42 in r {
///     print("found!");
/// }
/// ```
fn contains(range: RangeInclusive<int>, value: int) -> bool;

/// Return `true` if a key exists within the object map.
///
/// This function also drives the `in` operator.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
///
/// // The 'in' operator calls 'contains' in the background
/// if "c" in m {
///     print("found!");
/// }
/// ```
fn contains(map: Map, string: String) -> bool;

/// Return `true` if a value is found within the BLOB.
///
/// This function also drives the `in` operator.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// // The 'in' operator calls 'contains' in the background
/// if 3 in b {
///     print("found!");
/// }
/// ```
fn contains(blob: Blob, value: int) -> bool;

op !=(CallState, CallState) -> bool;

op ==(CallState, CallState) -> bool;

/// Abort the pending attended transfer.
fn abort_transfer(agent: Agent) -> ();

/// Answer the agent's incoming call.
/// # Example
/// ```rhai
/// await_until(|| assert(b.state).equals(State::Ringing), "15s");
/// b.accept();
/// ```
fn accept(agent: Agent) -> ();

/// Connect a headless baresip agent and return a handle.
///
/// **Config options** — `agent(name, #{ … })`:
///
/// | Field | Type | Description |
/// | --- | --- | --- |
/// | `username` | string · required | SIP user (registration / auth) |
/// | `domain` | string · required | SIP domain / registrar |
/// | `password` | string | auth password |
/// | `display_name` | string | caller display name |
/// | `transport` | string | `udp` (default), `tcp` or `tls` |
/// | `auth_user` | string | auth user, if it differs from `username` |
/// | `outbound` | string | outbound proxy URI |
/// | `stun_server` | string | STUN server, e.g. `stun:host:port` |
/// | `media_enc` | string | media encryption, e.g. `srtp`, `zrtp`, `dtls_srtp` |
/// | `regint` | int | re-registration interval (seconds); `0` disables |
/// | `mwi` | bool | subscribe to message-waiting indication |
/// | `dtmf_mode` | string | `"info"` for reliable headless DTMF (SIP INFO) |
/// | `headers` | map | extra SIP headers on the INVITE, e.g. `#{ "X-Foo": "bar" }` |
/// | `deflect_to` | string | deflect inbound calls with a 302 to this URI/number (toggle at runtime with `deflect()`/`stop_deflect()`) |
///
/// # Example
/// ```rhai
/// let a = agent("A", #{
///     username: env("A_USER"),
///     domain: env("SIP_DOMAIN"),
///     password: env("A_PASS"),
/// });
/// ```
fn agent(name: string, config: map) -> Agent;

/// Begin a fluent assertion on a value: `assert(x).equals(y)`, `.is_true()`,
/// `.greater_than(n)`, etc. Matchers chain (`.at_least(200).at_most(299)`)
/// and error (with a value-based message) on a mismatch. Asserting on a
/// getter auto-labels the log line (`assert(caller.state)` → `Caller state`,
/// `assert(res.status)` → `HTTP status`); `.describe(…)` overrides.
fn assert(actual: ?) -> Assertion;

/// Assert the (numeric) value is >= `n`.
fn at_least(a: Assertion, n: int) -> Assertion;

/// Assert the (numeric) value is <= `n`.
fn at_most(a: Assertion, n: int) -> Assertion;

/// Start an attended transfer: place a consultation call to another agent.
/// Complete it with `complete_transfer()` once that call is established.
/// # Example
/// ```rhai
/// callee.attended_transfer(target);   // consult `target`
/// await_until(|| assert(target.state).equals(State::Established));
/// callee.complete_transfer();         // connect caller and target
/// ```
fn attended_transfer(agent: Agent, target: Agent) -> ();

/// Start an attended transfer to a literal URI or bare number.
fn attended_transfer(agent: Agent, target: string) -> ();

/// Re-run the expression until its assertion holds or the default timeout
/// elapses; returns the body's value, so `.value()` can bind a verified value.
/// # Example
/// ```rhai
/// await_until(|| assert(a.registered).is_true());
/// ```
fn await_until(body: Fn) -> ?;

/// Like `await_until(body)` but with an explicit timeout, e.g. `"15s"`.
/// # Example
/// ```rhai
/// await_until(|| assert(b.state).equals(State::Ringing), "15s");
/// ```
fn await_until(body: Fn, within: string) -> ?;

/// Complete the pending attended transfer (REFER with Replaces).
fn complete_transfer(agent: Agent) -> ();

/// Assert the (string) value contains `needle`.
/// # Example
/// ```rhai
/// assert(a.header("User-Agent")).contains("baresip");
/// ```
fn contains(a: Assertion, needle: string) -> Assertion;

/// Set the default `await_until` timeout for the rest of the script (e.g. `"10s"`).
fn default_timeout(duration: string) -> ();

/// Deflect inbound calls with a 302 Moved Temporarily to another agent's AOR
/// (a Diversion header names the deflecting agent). Arm it *before* the
/// caller dials; stays active until `stop_deflect()`.
/// # Example
/// ```rhai
/// callee.deflect(target);     // future calls to `callee` go to `target`
/// caller.dial(callee);
/// await_until(|| assert(target.state).equals(State::Ringing), "15s");
/// ```
fn deflect(agent: Agent, target: Agent) -> ();

/// Deflect inbound calls (302) to a literal URI or bare number/extension.
fn deflect(agent: Agent, target: string) -> ();

/// Label this assertion so the log line names it: `assert(caller.registered)
/// .describe("caller registered").is_true()` → `caller registered: ✓ expect …`.
fn describe(a: Assertion, label: string) -> Assertion;

/// Dial another agent at its AOR.
/// # Example
/// ```rhai
/// a.dial(b);                 // dial agent B at its AOR
/// a.dial("+49301234567");    // …or a number/URI in A's domain
/// await_until(|| assert(b.state).equals(State::Ringing), "15s");
/// ```
fn dial(agent: Agent, target: Agent) -> ();

/// Dial a literal SIP URI, or a bare number/extension in the agent's own domain.
fn dial(agent: Agent, target: string) -> ();

/// Send DTMF tones (characters `0-9`, `*`, `#`, `A-D`) back-to-back.
/// # Example
/// ```rhai
/// a.dtmf("123#");
/// ```
fn dtmf(agent: Agent, digits: string) -> ();

/// Send DTMF tones with a pause between digits.
/// # Example
/// ```rhai
/// a.dtmf("123#", "200ms");
/// ```
fn dtmf(agent: Agent, digits: string, gap: string) -> ();

/// Read a variable: first from `--env-file`/`<scenario>.env`/`load_env`, then
/// the process environment. Errors if unset. Use it for per-env credentials.
/// # Example
/// ```rhai
/// let dom = env("SIP_DOMAIN");
/// let a = agent("A", #{ username: env("A_USER"), domain: dom, password: env("A_PASS") });
/// ```
fn env(name: string) -> string;

/// Assert the value equals `expected` (`is` is a reserved word in Rhai).
/// # Example
/// ```rhai
/// assert(a.state).equals(State::Established);
/// ```
fn equals(a: Assertion, expected: ?) -> Assertion;

/// Assert and report the status; errors on mismatch.
fn expect_status(response: HttpResponse, code: int) -> ();

/// A WAV-file audio source, for `send_audio`.
fn file(path: string) -> AudioSpec;

/// The raw request body.
fn get body(request: MockRequest) -> string;

/// The HTTP response body as a string.
fn get body(response: HttpResponse) -> string;

/// The request method (upper-case).
fn get method(request: MockRequest) -> string;

/// The remote party's display name, or `()` if absent.
/// # Returns: string?
fn get name(peer: Peer) -> ?;

/// The remote party's number (user-part of the URI), or `()`.
/// # Returns: string?
fn get number(peer: Peer) -> ?;

/// The request path.
fn get path(request: MockRequest) -> string;

/// The current call's remote party (the caller for an incoming call); read
/// `peer.uri` / `peer.number` / `peer.name` (each `()` if there's no call).
fn get peer(agent: Agent) -> Peer;

/// The port the server is listening on.
fn get port(mock: HttpMock) -> int;

/// The last closed call's reason (string), or `()` if none yet.
/// # Returns: string?
fn get reason(agent: Agent) -> ?;

/// Whether the agent's account is currently registered.
fn get registered(agent: Agent) -> bool;

/// The agent's current call phase: `Idle`, `Ringing` or `Established`.
fn get state(agent: Agent) -> CallState;

/// The HTTP response status code.
fn get status(response: HttpResponse) -> int;

/// SIP status code from the last closed call's reason (int, e.g. `603`),
/// or `()` if the reason isn't a SIP response (local hangup, reset, …).
/// # Returns: int?
fn get status_code(agent: Agent) -> ?;

/// The remote party's full URI (e.g. `sip:bob@example.com`), or `()`.
/// # Returns: string?
fn get uri(peer: Peer) -> ?;

/// The server's base URL, e.g. `http://127.0.0.1:8080`.
fn get url(mock: HttpMock) -> string;

/// Assert the (numeric) value is > `n`.
fn greater_than(a: Assertion, n: int) -> Assertion;

/// Hang up the agent's active call.
/// # Example
/// ```rhai
/// a.hangup();
/// await_until(|| assert(a.state).equals(State::Idle), "10s");
/// ```
fn hangup(agent: Agent) -> ();

/// Value of a header on a received INVITE (string), or `()` if absent.
/// # Returns: string?
fn header(agent: Agent, name: string) -> ?;

/// A request header value (case-insensitive), or `()` if absent.
/// # Returns: string?
fn header(request: MockRequest, name: string) -> ?;

/// A response header value (string), or `()` if absent.
/// # Returns: string?
fn header(response: HttpResponse, name: string) -> ?;

/// All received INVITE headers as a map (name → value); duplicates collapse,
/// use `header(name)` for a specific one.
fn headers(agent: Agent) -> map;

/// Put the active call on hold.
fn hold(agent: Agent) -> ();

/// Make an HTTP request and return the response.
/// # Example
/// ```rhai
/// let res = http("GET", env("API_URL") + "/calls");
/// res.expect_status(200);
/// ```
fn http(method: string, url: string) -> HttpResponse;

/// Make an HTTP request with options and return the response.
///
/// **Options** — `http(method, url, #{ … })`:
///
/// | Field | Type | Description |
/// | --- | --- | --- |
/// | `headers` | map | request headers, e.g. `#{ "Content-Type": "application/json" }` |
/// | `body` | string or map | request body; a map is encoded to JSON |
///
/// # Example
/// ```rhai
/// let res = http("POST", env("API_URL") + "/calls", #{
///     headers: #{ "Content-Type": "application/json" },
///     body: #{ to: "+49301234567" },
/// });
/// ```
fn http(method: string, url: string, options: map) -> HttpResponse;

/// A map of the agent's current state: name, aor, registered, state,
/// reason, status_code, calls. Handy to `print(...)` or assert on.
fn info(agent: Agent) -> map;

/// Assert the value is absent (`()`).
fn is_absent(a: Assertion) -> Assertion;

/// Assert the string/array/map value is empty.
fn is_empty(a: Assertion) -> Assertion;

/// Assert the value is `false`.
fn is_false(a: Assertion) -> Assertion;

/// Assert the string/array/map value is not empty.
fn is_not_empty(a: Assertion) -> Assertion;

/// Assert the value is present (not `()`), e.g. a received header.
fn is_present(a: Assertion) -> Assertion;

/// Assert the value is `true`.
/// # Example
/// ```rhai
/// assert(a.registered).is_true();
/// ```
fn is_true(a: Assertion) -> Assertion;

/// The whole JSON body as a native value (object→map, array, …).
fn json(response: HttpResponse) -> ?;

/// The value at a dotted JSON path in the body (object→map, array, number,
/// bool, `null`→`()`). Errors if the path is missing.
/// # Example
/// ```rhai
/// assert(req.json("call.from")).equals("+49301234567");
/// ```
fn json(request: MockRequest, path: string) -> ?;

/// The value at a dotted JSON path (e.g. `"data.id"`), typed: object→map,
/// array, number, bool, `null`→`()`. Errors if the path is missing.
/// # Example
/// ```rhai
/// assert(res.json("data.id")).equals(42);
/// ```
fn json(response: HttpResponse, path: string) -> ?;

/// Build a `200 application/json` response map from `body` (JSON-encoded),
/// for `respond`/`on`. `body` may be a map or an array.
/// # Example
/// ```rhai
/// hooks.respond("POST", "/voice", json_response(#{ actions: [ #{ type: "answer" } ] }));
/// ```
fn json_response(body: ?) -> map;

/// The most recent request on a `regex(...)` path (errors if none yet).
fn last_request(mock: HttpMock, path: PathPattern) -> MockRequest;

/// The most recent request on `path` (errors if none yet). Read it after
/// `await_until` confirms the webhook arrived.
/// # Example
/// ```rhai
/// let req = hooks.last_request("/voice");
/// assert(req.json("event")).equals("incoming_call");
/// ```
fn last_request(mock: HttpMock, path: string) -> MockRequest;

/// Assert the (numeric) value is < `n`.
fn less_than(a: Assertion, n: int) -> Assertion;

/// Load a dotenv file (`KEY=VALUE` lines) into `env(...)` for this scenario,
/// resolved relative to the scenario file. Later loads override earlier keys.
fn load_env(path: string) -> ();

/// Print a timestamped note to the scenario log (and the `--json` stream),
/// unlike `print` which writes a bare line.
fn log(message: string) -> ();

/// Assert the (string) value matches the regex `pattern`.
fn matches(a: Assertion, pattern: string) -> Assertion;

/// Start a mock HTTP server on a free port and return a handle. Stopped
/// automatically at the end of the scenario. Use `url` to point the system
/// under test at it, `respond`/`on` to define routes.
/// # Example
/// ```rhai
/// let hooks = mock_server();
/// hooks.on("POST", "/voice", |req| json_response(#{ actions: [ #{ type: "answer" } ] }));
/// http("PUT", env("API_URL") + "/config?webhook=" + hooks.url + "/voice");
/// ```
fn mock_server() -> HttpMock;

/// Start a mock HTTP server with config; stopped automatically at scenario
/// end. Omit `port` (or use `mock_server()`) for a free one.
///
/// **Config** — `mock_server(#{ … })`:
///
/// | Field | Type | Description |
/// | --- | --- | --- |
/// | `port` | int | port to bind (omit for a free one) |
///
/// # Example
/// ```rhai
/// let hooks = mock_server(#{ port: 8080 });
/// ```
fn mock_server(config: map) -> HttpMock;

/// Toggle mute on the active call.
/// # Example
/// ```rhai
/// a.mute(); // mute; call again to unmute
/// ```
fn mute(agent: Agent) -> ();

/// Assert the value does not equal `expected`.
fn not_equals(a: Assertion, expected: ?) -> Assertion;

/// Dynamic responder for a `regex(...)` path on any HTTP method.
fn on(mock: HttpMock, path: PathPattern, responder: Fn) -> ();

/// Dynamic responder for `path` on any HTTP method.
fn on(mock: HttpMock, path: string, responder: Fn) -> ();

/// Dynamic responder for `method` and a `regex(...)` path.
fn on(mock: HttpMock, method: string, path: PathPattern, responder: Fn) -> ();

/// Answer `method path` dynamically: the `|req|` closure receives the
/// `MockRequest` and returns a response map (e.g. `json_response(#{…})`).
/// `method` may be `"*"` for any method. The closure runs on a runtime
/// worker, so keep it pure (request → response): no agent verbs, no `wait`
/// — those block a worker thread.
/// # Example
/// ```rhai
/// hooks.on("POST", "/voice", |req| {
///     if req.json("event") == "incoming_call" {
///         json_response(#{ actions: [ #{ type: "answer" } ] })
///     } else {
///         json_response(#{ actions: [ #{ type: "hangup" } ] })
///     }
/// });
/// ```
fn on(mock: HttpMock, method: string, path: string, responder: Fn) -> ();

/// Run the given zero-arg closures concurrently and wait for all; returns
/// their results as an array, and fails if any task fails. Use it for
/// independent blocking work, e.g. `verify_audio` on several agents at once.
/// Tasks may share captured variables (each gets an independent snapshot,
/// so they can't race). Don't overlap `await_until` across tasks; its
/// silencing is global.
/// # Example
/// ```rhai
/// let results = parallel([
///     || http("GET", env("A_URL")),
///     || http("GET", env("B_URL")),
/// ]);
/// ```
fn parallel(tasks: array) -> array;

/// A query-string parameter value, or `()` if absent.
/// # Returns: string?
fn query(request: MockRequest, name: string) -> ?;

/// A regex path matcher for `respond`/`on`/`request_count`/… anchored to the
/// whole path (`/calls/.*` matches `/calls/123`). Errors on a bad pattern.
/// # Example
/// ```rhai
/// hooks.on(regex("/calls/.*"), |req| text_response("ok"));
/// ```
fn regex(pattern: string) -> PathPattern;

/// (Re-)register the agent's account.
/// # Example
/// ```rhai
/// a.register();
/// await_until(|| assert(a.registered).is_true(), "10s");
/// ```
fn register(agent: Agent) -> ();

/// How many requests arrived on a `regex(...)` path (any method).
fn request_count(mock: HttpMock, path: PathPattern) -> int;

/// How many requests arrived on `path` (any method). Poll it with
/// `await_until` to wait for a webhook.
/// # Example
/// ```rhai
/// await_until(|| assert(hooks.request_count("/voice")).equals(1), "10s");
/// ```
fn request_count(mock: HttpMock, path: string) -> int;

/// All requests on a `regex(...)` path, in arrival order, as `MockRequest`s.
fn requests(mock: HttpMock, path: PathPattern) -> array;

/// All requests received on `path`, in arrival order, as `MockRequest`s.
fn requests(mock: HttpMock, path: string) -> array;

/// Static response for a `regex(...)` path on any HTTP method.
fn respond(mock: HttpMock, path: PathPattern, response: map) -> ();

/// Static response for `path` on any HTTP method.
fn respond(mock: HttpMock, path: string, response: map) -> ();

/// Static response for `method` and a `regex(...)` path.
fn respond(mock: HttpMock, method: string, path: PathPattern, response: map) -> ();

/// Register a static response for `method path`: a map
/// `#{ status: 200, content_type: "…", headers: #{…}, body: <string|map> }`
/// (use `json_response`/`text_response` to build it). `method` may be `"*"`
/// for any method. Re-register to stage the next answer between webhooks.
/// # Example
/// ```rhai
/// hooks.respond("POST", "/voice", json_response(#{ actions: [ #{ type: "hangup" } ] }));
/// ```
fn respond(mock: HttpMock, method: string, path: string, response: map) -> ();

/// Answer inbound INVITEs with a custom SIP response (status + reason)
/// instead of accepting — e.g. `callee.respond_incoming(486, "Busy Here")`.
/// Arm before the caller dials; clear with `stop_deflect()`.
fn respond_incoming(agent: Agent, status: int, reason: string) -> ();

/// Custom response with extra headers, e.g.
/// `callee.respond_incoming(302, "Moved Temporarily", #{ "Contact": "<sip:bob@example.com>" })`.
/// Header values must not contain CR/LF.
fn respond_incoming(agent: Agent, status: int, reason: string, headers: map) -> ();

/// Resume a held call.
fn resume(agent: Agent) -> ();

/// Register a named scenario, run in isolation (fresh agents, torn down
/// after). The body may take the `setup()` context: `|ctx| { … }`.
/// # Example
/// ```rhai
/// scenario("answered call", |ctx| {
///     ctx.caller.dial(ctx.callee);
///     await_until(|| assert(ctx.callee.state).equals(State::Ringing), "15s");
///     ctx.callee.accept();
/// });
/// ```
fn scenario(name: string, body: Fn) -> ();

/// Register a scenario with options `#{ tags: ["smoke"], skip: true|"reason",
/// only: true }`. `--tag`/`--exclude-tag` filter by tag; a skipped scenario is
/// reported but not run; if any scenario sets `only`, only those run.
/// # Example
/// ```rhai
/// scenario("smoke: answered", #{ tags: ["smoke"] }, |ctx| {
///     ctx.caller.dial(ctx.callee);
///     ctx.callee.accept();
/// });
/// ```
fn scenario(name: string, options: map, body: Fn) -> ();

/// Switch the agent's active-call audio source: `tone(Hz)`, `file(path)` or `silent()`.
/// # Example
/// ```rhai
/// a.send_audio(tone(440));         // play a 440 Hz tone
/// a.send_audio(file("prompt.wav"));
/// ```
fn send_audio(agent: Agent, source: AudioSpec) -> ();

/// Run before each scenario; its return value is passed to the scenario
/// (and teardown) as `ctx`. Typically creates and registers the agents.
/// # Example
/// ```rhai
/// setup(|| {
///     let caller = agent("Caller", #{ username: env("A_USER"), domain: env("SIP_DOMAIN"), password: env("A_PASS") });
///     caller.register();
///     #{ caller: caller }
/// });
/// ```
fn setup(body: Fn) -> ();

/// A silent audio source (stop sending), for `send_audio`.
fn silent() -> AudioSpec;

/// Skip the current scenario at runtime (reported, not failed).
fn skip() -> ();

/// Skip the current scenario at runtime with a reason (reported, not failed).
/// # Example
/// ```rhai
/// if env("STAGE") != "prod" { skip("prod only") }
/// ```
fn skip(reason: string) -> ();

/// Stop the server now (it otherwise stops automatically at scenario end).
fn stop(mock: HttpMock) -> ();

/// Stop deflecting / clear any armed response — inbound calls are accepted again.
fn stop_deflect(agent: Agent) -> ();

/// Run after each scenario (even on failure); receives the `setup` context.
/// # Example
/// ```rhai
/// teardown(|ctx| { ctx.caller.hangup(); });
/// ```
fn teardown(body: Fn) -> ();

/// Build a `200 text/plain` response map from `body`, for `respond`/`on`.
fn text_response(body: string) -> map;

/// The agent's current state as a JSON string (for `log(...)`/debugging).
fn to_json(agent: Agent) -> string;

fn to_string(_: Peer) -> string;

/// The call state as a string.
fn to_string(state: CallState) -> string;

/// A sine-tone audio source at the given frequency (Hz), for `send_audio`.
/// # Example
/// ```rhai
/// a.send_audio(tone(440));
/// ```
fn tone(freq: int) -> AudioSpec;

/// Blind-transfer (REFER) the active call to another agent's AOR.
/// # Example
/// ```rhai
/// callee.transfer(target); // hand the caller off to `target`
/// ```
fn transfer(agent: Agent, target: Agent) -> ();

/// Blind-transfer (REFER) the active call to a literal URI or bare number.
fn transfer(agent: Agent, target: string) -> ();

/// A fresh random UUID string.
fn uuid() -> string;

/// The value under assertion, so a verified value can be bound.
/// # Example
/// ```rhai
/// let id = await_until(|| assert(callee.header("X-Id")).is_present().value());
/// ```
fn value(a: Assertion) -> ?;

/// Assert the agent is receiving a tone at `freq` Hz within the window (Goertzel).
/// # Example
/// ```rhai
/// a.send_audio(tone(440));
/// b.verify_audio(440, "5s"); // b hears A's 440 Hz tone
/// ```
fn verify_audio(agent: Agent, freq: int, within: string) -> ();

/// Assert two-way audio between two agents (a→b then b→a) at 1000 Hz.
/// # Example
/// ```rhai
/// caller.verify_audio_connection(callee);
/// ```
fn verify_audio_connection(a: Agent, b: Agent) -> ();

/// Hold for N seconds; FAILS if a call that is established at the start drops.
/// # Example
/// ```rhai
/// wait(3); // the call must stay up for 3s
/// ```
fn wait(seconds: int) -> ();

op !(bool) -> bool;

/// Return `true` if two arrays are not-equal (i.e. any element not equal or not in the same order).
///
/// The operator `==` is used to compare elements and must be defined,
/// otherwise `false` is assumed.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
/// let y = [1, 2, 3, 4, 5];
/// let z = [1, 2, 3, 4];
///
/// print(x != y);      // prints false
///
/// print(x != z);      // prints true
/// ```
op !=(array, array) -> bool;

/// Return `true` if two object maps are not equal (i.e. at least one property value is not equal).
///
/// The operator `==` is used to compare property values and must be defined,
/// otherwise `false` is assumed.
///
/// # Example
///
/// ```rhai
/// let m1 = #{a:1, b:2, c:3};
/// let m2 = #{a:1, b:2, c:3};
/// let m3 = #{a:1, c:3};
///
/// print(m1 != m2);        // prints false
///
/// print(m1 != m3);        // prints true
/// ```
op !=(map, map) -> bool;

/// Return `true` if two timestamps are not equal.
op !=(timestamp, timestamp) -> bool;

op !=(int, f32) -> bool;

op !=(f32, int) -> bool;

op !=(f32, f32) -> bool;

op !=(i128, i128) -> bool;

op !=(i16, i16) -> bool;

op !=(i32, i32) -> bool;

op !=(i8, i8) -> bool;

op !=(u128, u128) -> bool;

op !=(u16, u16) -> bool;

op !=(u32, u32) -> bool;

op !=(u64, u64) -> bool;

op !=(u8, u8) -> bool;

op %(int, f32) -> f32;

op %(f32, int) -> f32;

op %(f32, f32) -> f32;

op %(i128, i128) -> i128;

op %(i16, i16) -> i16;

op %(i32, i32) -> i32;

op %(i8, i8) -> i8;

op %(u128, u128) -> u128;

op %(u16, u16) -> u16;

op %(u32, u32) -> u32;

op %(u64, u64) -> u64;

op %(u8, u8) -> u8;

op &(i128, i128) -> i128;

op &(i16, i16) -> i16;

op &(i32, i32) -> i32;

op &(i8, i8) -> i8;

op &(u128, u128) -> u128;

op &(u16, u16) -> u16;

op &(u32, u32) -> u32;

op &(u64, u64) -> u64;

op &(u8, u8) -> u8;

op *(int, f32) -> f32;

op *(f32, int) -> f32;

op *(f32, f32) -> f32;

op *(i128, i128) -> i128;

op *(i16, i16) -> i16;

op *(i32, i32) -> i32;

op *(i8, i8) -> i8;

op *(u128, u128) -> u128;

op *(u16, u16) -> u16;

op *(u32, u32) -> u32;

op *(u64, u64) -> u64;

op *(u8, u8) -> u8;

op **(f32, int) -> f32;

op **(f32, f32) -> f32;

op **(i128, int) -> i128;

op **(i16, int) -> i16;

op **(i32, int) -> i32;

op **(i8, int) -> i8;

op **(u128, int) -> u128;

op **(u16, int) -> u16;

op **(u32, int) -> u32;

op **(u64, int) -> u64;

op **(u8, int) -> u8;

op +(int) -> int;

op +(f32) -> f32;

op +(float) -> float;

op +(i128) -> i128;

op +(i16) -> i16;

op +(i32) -> i32;

op +(i8) -> i8;

/// Combine two arrays into a new array and return it.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
/// let y = [true, 'x'];
///
/// print(x + y);   // prints "[1, 2, 3, true, 'x']"
///
/// print(x);       // prints "[1, 2, 3"
/// ```
op +(array, array) -> array;

op +(char, string) -> string;

op +(?, string) -> string;

op +((), string) -> string;

/// Make a copy of the object map, add all property values of another object map
/// (existing property values of the same names are replaced), then returning it.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
/// let n = #{a: 42, d:0};
///
/// print(m + n);       // prints "#{a:42, b:2, c:3, d:0}"
///
/// print(m);           // prints "#{a:1, b:2, c:3}"
/// ```
op +(map, map) -> map;

op +(string, string) -> string;

op +(string, ?) -> string;

op +(string, char) -> string;

op +(string, ()) -> string;

op +(string, blob) -> string;

/// Add the specified number of `seconds` to the timestamp and return it as a new timestamp.
op +(timestamp, float) -> Instant;

/// Add the specified number of `seconds` to the timestamp and return it as a new timestamp.
op +(timestamp, int) -> Instant;

op +(blob, string) -> string;

op +(int, f32) -> f32;

op +(f32, int) -> f32;

op +(f32, f32) -> f32;

op +(i128, i128) -> i128;

op +(i16, i16) -> i16;

op +(i32, i32) -> i32;

op +(i8, i8) -> i8;

op +(u128, u128) -> u128;

op +(u16, u16) -> u16;

op +(u32, u32) -> u32;

op +(u64, u64) -> u64;

op +(u8, u8) -> u8;

/// Add all property values of another object map into the object map.
/// Existing property values of the same names are replaced.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
/// let n = #{a: 42, d:0};
///
/// m.mixin(n);
///
/// print(m);       // prints "#{a:42, b:2, c:3, d:0}"
/// ```
op +=(map, map) -> ();

op +=(string, string) -> ();

op +=(string, char) -> ();

op +=(string, ()) -> ();

op +=(string, ?) -> ();

op +=(string, blob) -> ();

/// Add the specified number of `seconds` to the timestamp.
op +=(timestamp, float) -> ();

/// Add the specified number of `seconds` to the timestamp.
op +=(timestamp, int) -> ();

op -(int) -> int;

op -(f32) -> f32;

op -(float) -> float;

op -(i128) -> i128;

op -(i16) -> i16;

op -(i32) -> i32;

op -(i8) -> i8;

/// Return the number of seconds between two timestamps.
op -(timestamp, timestamp) -> RhaiResult;

/// Subtract the specified number of `seconds` from the timestamp and return it as a new timestamp.
op -(timestamp, float) -> Instant;

/// Subtract the specified number of `seconds` from the timestamp and return it as a new timestamp.
op -(timestamp, int) -> Instant;

op -(int, f32) -> f32;

op -(f32, int) -> f32;

op -(f32, f32) -> f32;

op -(i128, i128) -> i128;

op -(i16, i16) -> i16;

op -(i32, i32) -> i32;

op -(i8, i8) -> i8;

op -(u128, u128) -> u128;

op -(u16, u16) -> u16;

op -(u32, u32) -> u32;

op -(u64, u64) -> u64;

op -(u8, u8) -> u8;

/// Subtract the specified number of `seconds` from the timestamp.
op -=(timestamp, float) -> ();

/// Subtract the specified number of `seconds` from the timestamp.
op -=(timestamp, int) -> ();

op /(int, f32) -> f32;

op /(f32, int) -> f32;

op /(f32, f32) -> f32;

op /(i128, i128) -> i128;

op /(i16, i16) -> i16;

op /(i32, i32) -> i32;

op /(i8, i8) -> i8;

op /(u128, u128) -> u128;

op /(u16, u16) -> u16;

op /(u32, u32) -> u32;

op /(u64, u64) -> u64;

op /(u8, u8) -> u8;

/// Return `true` if the first timestamp is earlier than the second.
op <(timestamp, timestamp) -> bool;

op <(int, f32) -> bool;

op <(f32, int) -> bool;

op <(f32, f32) -> bool;

op <(i128, i128) -> bool;

op <(i16, i16) -> bool;

op <(i32, i32) -> bool;

op <(i8, i8) -> bool;

op <(u128, u128) -> bool;

op <(u16, u16) -> bool;

op <(u32, u32) -> bool;

op <(u64, u64) -> bool;

op <(u8, u8) -> bool;

op <<(i128, int) -> i128;

op <<(i16, int) -> i16;

op <<(i32, int) -> i32;

op <<(i8, int) -> i8;

op <<(u128, int) -> u128;

op <<(u16, int) -> u16;

op <<(u32, int) -> u32;

op <<(u64, int) -> u64;

op <<(u8, int) -> u8;

/// Return `true` if the first timestamp is earlier than or equals to the second.
op <=(timestamp, timestamp) -> bool;

op <=(int, f32) -> bool;

op <=(f32, int) -> bool;

op <=(f32, f32) -> bool;

op <=(i128, i128) -> bool;

op <=(i16, i16) -> bool;

op <=(i32, i32) -> bool;

op <=(i8, i8) -> bool;

op <=(u128, u128) -> bool;

op <=(u16, u16) -> bool;

op <=(u32, u32) -> bool;

op <=(u64, u64) -> bool;

op <=(u8, u8) -> bool;

/// Return `true` if two arrays are equal (i.e. all elements are equal and in the same order).
///
/// The operator `==` is used to compare elements and must be defined,
/// otherwise `false` is assumed.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
/// let y = [1, 2, 3, 4, 5];
/// let z = [1, 2, 3, 4];
///
/// print(x == y);      // prints true
///
/// print(x == z);      // prints false
/// ```
op ==(array, array) -> bool;

/// Return `true` if two object maps are equal (i.e. all property values are equal).
///
/// The operator `==` is used to compare property values and must be defined,
/// otherwise `false` is assumed.
///
/// # Example
///
/// ```rhai
/// let m1 = #{a:1, b:2, c:3};
/// let m2 = #{a:1, b:2, c:3};
/// let m3 = #{a:1, c:3};
///
/// print(m1 == m2);        // prints true
///
/// print(m1 == m3);        // prints false
/// ```
op ==(map, map) -> bool;

/// Return `true` if two timestamps are equal.
op ==(timestamp, timestamp) -> bool;

op ==(int, f32) -> bool;

op ==(f32, int) -> bool;

op ==(f32, f32) -> bool;

op ==(i128, i128) -> bool;

op ==(i16, i16) -> bool;

op ==(i32, i32) -> bool;

op ==(i8, i8) -> bool;

op ==(u128, u128) -> bool;

op ==(u16, u16) -> bool;

op ==(u32, u32) -> bool;

op ==(u64, u64) -> bool;

op ==(u8, u8) -> bool;

/// Return `true` if the first timestamp is later than the second.
op >(timestamp, timestamp) -> bool;

op >(int, f32) -> bool;

op >(f32, int) -> bool;

op >(f32, f32) -> bool;

op >(i128, i128) -> bool;

op >(i16, i16) -> bool;

op >(i32, i32) -> bool;

op >(i8, i8) -> bool;

op >(u128, u128) -> bool;

op >(u16, u16) -> bool;

op >(u32, u32) -> bool;

op >(u64, u64) -> bool;

op >(u8, u8) -> bool;

/// Return `true` if the first timestamp is later than or equals to the second.
op >=(timestamp, timestamp) -> bool;

op >=(int, f32) -> bool;

op >=(f32, int) -> bool;

op >=(f32, f32) -> bool;

op >=(i128, i128) -> bool;

op >=(i16, i16) -> bool;

op >=(i32, i32) -> bool;

op >=(i8, i8) -> bool;

op >=(u128, u128) -> bool;

op >=(u16, u16) -> bool;

op >=(u32, u32) -> bool;

op >=(u64, u64) -> bool;

op >=(u8, u8) -> bool;

op >>(i128, int) -> i128;

op >>(i16, int) -> i16;

op >>(i32, int) -> i32;

op >>(i8, int) -> i8;

op >>(u128, int) -> u128;

op >>(u16, int) -> u16;

op >>(u32, int) -> u32;

op >>(u64, int) -> u64;

op >>(u8, int) -> u8;

/// Return the natural number _e_.
fn E() -> float;

/// Return the number π.
fn PI() -> float;

op ^(i128, i128) -> i128;

op ^(i16, i16) -> i16;

op ^(i32, i32) -> i32;

op ^(i8, i8) -> i8;

op ^(u128, u128) -> u128;

op ^(u16, u16) -> u16;

op ^(u32, u32) -> u32;

op ^(u64, u64) -> u64;

op ^(u8, u8) -> u8;

/// Return the absolute value of the number.
fn abs(x: int) -> int;

/// Return the absolute value of the floating-point number.
fn abs(x: f32) -> f32;

/// Return the absolute value of the floating-point number.
fn abs(x: float) -> float;

/// Return the absolute value of the number.
fn abs(x: i128) -> i128;

/// Return the absolute value of the number.
fn abs(x: i16) -> i16;

/// Return the absolute value of the number.
fn abs(x: i32) -> i32;

/// Return the absolute value of the number.
fn abs(x: i8) -> i8;

/// Return the arc-cosine of the floating-point number, in radians.
fn acos(x: float) -> float;

/// Return the arc-hyperbolic-cosine of the floating-point number, in radians.
fn acosh(x: float) -> float;

/// Return `true` if all elements in the array return `true` when applied a function named by `filter`.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.all(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `filter` must exist taking these parameters:
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.all(|v| v > 3));        // prints false
///
/// print(x.all(|v| v > 1));        // prints true
///
/// print(x.all(|v, i| i > v));     // prints false
/// ```
fn all(array: array, filter: string) -> bool;

/// Return `true` if all elements in the array return `true` when applied the `filter` function.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `filter` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.all(|v| v > 3));        // prints false
///
/// print(x.all(|v| v > 1));        // prints true
///
/// print(x.all(|v, i| i > v));     // prints false
/// ```
fn all(array: array, filter: Fn) -> bool;

/// Add all the elements of another array to the end of the array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
/// let y = [true, 'x'];
///
/// x.append(y);
///
/// print(x);       // prints "[1, 2, 3, true, 'x']"
/// ```
fn append(array: array, new_array: array) -> ();

/// Add another BLOB to the end of the BLOB.
///
/// # Example
///
/// ```rhai
/// let b1 = blob(5, 0x42);
/// let b2 = blob(3, 0x11);
///
/// b1.push(b2);
///
/// print(b1);      // prints "[4242424242111111]"
/// ```
fn append(blob1: blob, blob2: blob) -> ();

/// Add a character (as UTF-8 encoded byte-stream) to the end of the BLOB
///
/// # Example
///
/// ```rhai
/// let b = blob(5, 0x42);
///
/// b.append('!');
///
/// print(b);       // prints "[424242424221]"
/// ```
fn append(blob: blob, character: char) -> ();

/// Add a string (as UTF-8 encoded byte-stream) to the end of the BLOB
///
/// # Example
///
/// ```rhai
/// let b = blob(5, 0x42);
///
/// b.append("hello");
///
/// print(b);       // prints "[424242424268656c 6c6f]"
/// ```
fn append(blob: blob, string: string) -> ();

/// Add a new byte `value` to the end of the BLOB.
///
/// Only the lower 8 bits of the `value` are used; all other bits are ignored.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b.push(0x42);
///
/// print(b);       // prints "[42]"
/// ```
fn append(blob: blob, value: int) -> ();

fn append(string: string, item: ?) -> ();

fn append(string: string, utf8: blob) -> ();

/// Convert the BLOB into a string.
///
/// The byte stream must be valid UTF-8, otherwise an error is raised.
///
/// # Example
///
/// ```rhai
/// let b = blob(5, 0x42);
///
/// let x = b.as_string();
///
/// print(x);       // prints "FFFFF"
/// ```
fn as_string(blob: blob) -> String;

/// Return the arc-sine of the floating-point number, in radians.
fn asin(x: float) -> float;

/// Return the arc-hyperbolic-sine of the floating-point number, in radians.
fn asinh(x: float) -> float;

/// Return the arc-tangent of the floating-point number, in radians.
fn atan(x: float) -> float;

/// Return the arc-tangent of the floating-point numbers `x` and `y`, in radians.
fn atan(x: float, y: float) -> float;

/// Return the arc-hyperbolic-tangent of the floating-point number, in radians.
fn atanh(x: float) -> float;

/// Return an iterator over all the bits in the number.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// for bit in x.bits() {
///     print(bit);
/// }
/// ```
fn bits(value: int) -> BitRange;

/// Return an iterator over the bits in the number starting from the specified `start` position.
///
/// If `start` < 0, position counts from the MSB (Most Significant Bit)>.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// for bit in x.bits(10) {
///     print(bit);
/// }
/// ```
fn bits(value: int, from: int) -> BitRange;

/// Return an iterator over an exclusive range of bits in the number.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// for bit in x.bits(10..24) {
///     print(bit);
/// }
/// ```
fn bits(value: int, range: range) -> BitRange;

/// Return an iterator over an inclusive range of bits in the number.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// for bit in x.bits(10..=23) {
///     print(bit);
/// }
/// ```
fn bits(value: int, range: range=) -> BitRange;

/// Return an iterator over a portion of bits in the number.
///
/// * If `start` < 0, position counts from the MSB (Most Significant Bit)>.
/// * If `len` ≤ 0, an empty iterator is returned.
/// * If `start` position + `len` ≥ length of string, all bits of the number after the `start` position are iterated.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// for bit in x.bits(10, 8) {
///     print(bit);
/// }
/// ```
fn bits(value: int, from: int, len: int) -> BitRange;

/// Return a new, empty BLOB.
fn blob() -> blob;

/// Return a new BLOB of the specified length, filled with zeros.
///
/// If `len` ≤ 0, an empty BLOB is returned.
///
/// # Example
///
/// ```rhai
/// let b = blob(10);
///
/// print(b);       // prints "[0000000000000000 0000]"
/// ```
fn blob(len: int) -> Blob;

/// Return a new BLOB of the specified length, filled with copies of the initial `value`.
///
/// If `len` ≤ 0, an empty BLOB is returned.
///
/// Only the lower 8 bits of the initial `value` are used; all other bits are ignored.
///
/// # Example
///
/// ```rhai
/// let b = blob(10, 0x42);
///
/// print(b);       // prints "[4242424242424242 4242]"
/// ```
fn blob(len: int, value: int) -> Blob;

/// Return the length of the string, in number of bytes used to store it in UTF-8 encoding.
///
/// # Example
///
/// ```rhai
/// let text = "朝には紅顔ありて夕べには白骨となる";
///
/// print(text.bytes);      // prints 51
/// ```
fn bytes(string: string) -> int;

/// Return the smallest whole number larger than or equals to the floating-point number.
fn ceiling(x: float) -> float;

/// Return an iterator over the characters in the string.
///
/// # Example
///
/// ```rhai
/// for ch in "hello, world!".chars() {
///     print(ch);
/// }
/// ```
fn chars(string: string) -> CharsStream;

/// Return an iterator over an exclusive range of characters in the string.
///
/// # Example
///
/// ```rhai
/// for ch in "hello, world!".chars(2..5) {
///     print(ch);
/// }
/// ```
fn chars(string: string, range: range) -> CharsStream;

/// Return an iterator over an inclusive range of characters in the string.
///
/// # Example
///
/// ```rhai
/// for ch in "hello, world!".chars(2..=6) {
///     print(ch);
/// }
/// ```
fn chars(string: string, range: range=) -> CharsStream;

/// Return an iterator over the characters in the string starting from the `start` position.
///
/// * If `start` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `start` < -length of string, position counts from the beginning of the string.
/// * If `start` ≥ length of string, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// for ch in "hello, world!".chars(2) {
///     print(ch);
/// }
/// ```
fn chars(string: string, start: int) -> CharsStream;

/// Return an iterator over a portion of characters in the string.
///
/// * If `start` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `start` < -length of string, position counts from the beginning of the string.
/// * If `start` ≥ length of string, an empty iterator is returned.
/// * If `len` ≤ 0, an empty iterator is returned.
/// * If `start` position + `len` ≥ length of string, all characters of the string after the `start` position are iterated.
///
/// # Example
///
/// ```rhai
/// for ch in "hello, world!".chars(2, 4) {
///     print(ch);
/// }
/// ```
fn chars(string: string, start: int, len: int) -> CharsStream;

/// Cut off the head of the array, leaving a tail of the specified length.
///
/// * If `len` ≤ 0, the array is cleared.
/// * If `len` ≥ length of array, the array is not modified.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// x.chop(3);
///
/// print(x);       // prints "[3, 4, 5]"
///
/// x.chop(10);
///
/// print(x);       // prints "[3, 4, 5]"
/// ```
fn chop(array: array, len: int) -> ();

/// Cut off the head of the BLOB, leaving a tail of the specified length.
///
/// * If `len` ≤ 0, the BLOB is cleared.
/// * If `len` ≥ length of BLOB, the BLOB is not modified.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// b.chop(3);
///
/// print(b);           // prints "[030405]"
///
/// b.chop(10);
///
/// print(b);           // prints "[030405]"
/// ```
fn chop(blob: blob, len: int) -> ();

/// Clear the array.
fn clear(array: array) -> ();

/// Clear the BLOB.
fn clear(blob: blob) -> ();

/// Clear the object map.
fn clear(map: map) -> ();

/// Clear the string, making it empty.
fn clear(string: string) -> ();

/// Return `true` if the array contains an element that equals `value`.
///
/// The operator `==` is used to compare elements with `value` and must be defined,
/// otherwise `false` is assumed.
///
/// This function also drives the `in` operator.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// // The 'in' operator calls 'contains' in the background
/// if 4 in x {
///     print("found!");
/// }
/// ```
fn contains(array: array, value: ?) -> bool;

/// Return `true` if the BLOB contains a specified byte value.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.contains('h'));      // prints true
///
/// print(text.contains('x'));      // prints false
/// ```
fn contains(blob: blob, value: int) -> bool;

/// Returns `true` if the object map contains a specified property.
///
/// # Example
///
/// ```rhai
/// let m = #{a: 1, b: 2, c: 3};
///
/// print(m.contains("b"));     // prints true
///
/// print(m.contains("x"));     // prints false
/// ```
fn contains(map: map, property: string) -> bool;

/// Return `true` if the range contains a specified value.
fn contains(range: range, value: int) -> bool;

/// Return `true` if the range contains a specified value.
fn contains(range: range=, value: int) -> bool;

/// Return `true` if the string contains a specified character.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.contains('h'));      // prints true
///
/// print(text.contains('x'));      // prints false
/// ```
fn contains(string: string, character: char) -> bool;

/// Return `true` if the string contains a specified string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.contains("hello"));  // prints true
///
/// print(text.contains("hey"));    // prints false
/// ```
fn contains(string: string, match_string: string) -> bool;

/// Return the cosine of the floating-point number in radians.
fn cos(x: float) -> float;

/// Return the hyperbolic cosine of the floating-point number in radians.
fn cosh(x: float) -> float;

/// Remove all characters from the string except those within an exclusive `range`.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// text.crop(2..8);
///
/// print(text);        // prints "llo, w"
/// ```
fn crop(string: string, range: range) -> ();

/// Remove all characters from the string except those within an inclusive `range`.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// text.crop(2..=8);
///
/// print(text);        // prints "llo, wo"
/// ```
fn crop(string: string, range: range=) -> ();

/// Remove all characters from the string up to the `start` position.
///
/// * If `start` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `start` < -length of string, the string is not modified.
/// * If `start` ≥ length of string, the entire string is cleared.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// text.crop(5);
///
/// print(text);            // prints ", world!"
///
/// text.crop(-3);
///
/// print(text);            // prints "ld!"
/// ```
fn crop(string: string, start: int) -> ();

/// Remove all characters from the string except those within a range.
///
/// * If `start` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `start` < -length of string, position counts from the beginning of the string.
/// * If `start` ≥ length of string, the entire string is cleared.
/// * If `len` ≤ 0, the entire string is cleared.
/// * If `start` position + `len` ≥ length of string, only the portion of the string after the `start` position is retained.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// text.crop(2, 8);
///
/// print(text);        // prints "llo, wor"
///
/// text.crop(-5, 3);
///
/// print(text);        // prints ", w"
/// ```
fn crop(string: string, start: int, len: int) -> ();

/// Return the empty string.
op debug() -> string;

/// Convert the array into a string.
op debug(array) -> string;

/// Convert the string into debug format.
op debug(char) -> string;

/// Convert the function pointer into a string in debug format.
op debug(Fn) -> string;

/// Convert the value of the `item` into a string in debug format.
op debug(?) -> string;

/// Convert the object map into a string.
op debug(map) -> string;

/// Convert the value of `number` into a string.
op debug(f32) -> string;

/// Convert the value of `number` into a string.
op debug(float) -> string;

/// Convert the string into debug format.
op debug(string) -> string;

/// Convert the unit into a string in debug format.
op debug(()) -> string;

/// Convert the boolean value into a string in debug format.
op debug(bool) -> string;

/// Remove duplicated _consecutive_ elements from the array.
///
/// The operator `==` is used to compare elements and must be defined,
/// otherwise `false` is assumed.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 2, 2, 3, 4, 3, 3, 2, 1];
///
/// x.dedup();
///
/// print(x);       // prints "[1, 2, 3, 4, 3, 2, 1]"
/// ```
fn dedup(array: array) -> ();

/// Remove duplicated _consecutive_ elements from the array that return `true` when applied a
/// function named by `comparer`.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.dedup(Fn("fn_name"))` instead.
///
/// No element is removed if the correct `comparer` function does not exist.
///
/// # Function Parameters
///
/// * `element1`: copy of the current array element to compare
/// * `element2`: copy of the next array element to compare
///
/// ## Return Value
///
/// `true` if `element1 == element2`, otherwise `false`.
///
/// # Example
///
/// ```rhai
/// fn declining(a, b) { a >= b }
///
/// let x = [1, 2, 2, 2, 3, 1, 2, 3, 4, 3, 3, 2, 1];
///
/// x.dedup("declining");
///
/// print(x);       // prints "[1, 2, 3, 4]"
/// ```
fn dedup(array: array, comparer: string) -> ();

/// Remove duplicated _consecutive_ elements from the array that return `true` when applied the
/// `comparer` function.
///
/// No element is removed if the correct `comparer` function does not exist.
///
/// # Function Parameters
///
/// * `element1`: copy of the current array element to compare
/// * `element2`: copy of the next array element to compare
///
/// ## Return Value
///
/// `true` if `element1 == element2`, otherwise `false`.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 2, 2, 3, 1, 2, 3, 4, 3, 3, 2, 1];
///
/// x.dedup(|a, b| a >= b);
///
/// print(x);       // prints "[1, 2, 3, 4]"
/// ```
fn dedup(array: array, comparer: Fn) -> ();

/// Remove all elements in the array that returns `true` when applied a function named by `filter`
/// and return them as a new array.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.drain(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `filter` must exist taking these parameters:
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn small(x) { x < 3 }
///
/// fn screen(x, i) { x + i > 5 }
///
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.drain("small");
///
/// print(x);       // prints "[3, 4, 5]"
///
/// print(y);       // prints "[1, 2]"
///
/// let z = x.drain("screen");
///
/// print(x);       // prints "[3, 4]"
///
/// print(z);       // prints "[5]"
/// ```
fn drain(array: array, filter: string) -> Array;

/// Remove all elements in the array that returns `true` when applied the `filter` function and
/// return them as a new array.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.drain(|v| v < 3);
///
/// print(x);       // prints "[3, 4, 5]"
///
/// print(y);       // prints "[1, 2]"
///
/// let z = x.drain(|v, i| v + i > 5);
///
/// print(x);       // prints "[3, 4]"
///
/// print(z);       // prints "[5]"
/// ```
fn drain(array: array, filter: Fn) -> Array;

/// Remove all elements in the array within an exclusive `range` and return them as a new array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.drain(1..3);
///
/// print(x);       // prints "[1, 4, 5]"
///
/// print(y);       // prints "[2, 3]"
///
/// let z = x.drain(2..3);
///
/// print(x);       // prints "[1, 4]"
///
/// print(z);       // prints "[5]"
/// ```
fn drain(array: array, range: range) -> array;

/// Remove all elements in the array within an inclusive `range` and return them as a new array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.drain(1..=2);
///
/// print(x);       // prints "[1, 4, 5]"
///
/// print(y);       // prints "[2, 3]"
///
/// let z = x.drain(2..=2);
///
/// print(x);       // prints "[1, 4]"
///
/// print(z);       // prints "[5]"
/// ```
fn drain(array: array, range: range=) -> array;

/// Remove all bytes in the BLOB within an exclusive `range` and return them as a new BLOB.
///
/// # Example
///
/// ```rhai
/// let b1 = blob();
///
/// b1 += 1; b1 += 2; b1 += 3; b1 += 4; b1 += 5;
///
/// let b2 = b1.drain(1..3);
///
/// print(b1);      // prints "[010405]"
///
/// print(b2);      // prints "[0203]"
///
/// let b3 = b1.drain(2..3);
///
/// print(b1);      // prints "[0104]"
///
/// print(b3);      // prints "[05]"
/// ```
fn drain(blob: blob, range: range) -> blob;

/// Remove all bytes in the BLOB within an inclusive `range` and return them as a new BLOB.
///
/// # Example
///
/// ```rhai
/// let b1 = blob();
///
/// b1 += 1; b1 += 2; b1 += 3; b1 += 4; b1 += 5;
///
/// let b2 = b1.drain(1..=2);
///
/// print(b1);      // prints "[010405]"
///
/// print(b2);      // prints "[0203]"
///
/// let b3 = b1.drain(2..=2);
///
/// print(b1);      // prints "[0104]"
///
/// print(b3);      // prints "[05]"
/// ```
fn drain(blob: blob, range: range=) -> blob;

/// Remove all elements in the object map that return `true` when applied the `filter` function and
/// return them as a new object map.
///
/// # Function Parameters
///
/// * `key`: current key
/// * `value` _(optional)_: copy of element (bound to `this` if omitted)
///
/// # Example
///
/// ```rhai
/// let x = #{a:1, b:2, c:3, d:4, e:5};
///
/// let y = x.drain(|k| this < 3);
///
/// print(x);       // prints #{"c":3, "d":4, "e":5]
///
/// print(y);       // prints #{"a":1, "b"2}
///
/// let z = x.drain(|k, v| k == "c" || v >= 5);
///
/// print(x);       // prints #{"d":4}
///
/// print(z);       // prints #{"c":3, "e":5}
/// ```
fn drain(map: map, filter: Fn) -> Map;

/// Remove all elements within a portion of the array and return them as a new array.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, no element is removed and an empty array is returned.
/// * If `len` ≤ 0, no element is removed and an empty array is returned.
/// * If `start` position + `len` ≥ length of array, entire portion of the array after the `start` position is removed and returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.drain(1, 2);
///
/// print(x);       // prints "[1, 4, 5]"
///
/// print(y);       // prints "[2, 3]"
///
/// let z = x.drain(-1, 1);
///
/// print(x);       // prints "[1, 4]"
///
/// print(z);       // prints "[5]"
/// ```
fn drain(array: array, start: int, len: int) -> array;

/// Remove all bytes within a portion of the BLOB and return them as a new BLOB.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, nothing is removed and an empty BLOB is returned.
/// * If `len` ≤ 0, nothing is removed and an empty BLOB is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is removed and returned.
///
/// # Example
///
/// ```rhai
/// let b1 = blob();
///
/// b1 += 1; b1 += 2; b1 += 3; b1 += 4; b1 += 5;
///
/// let b2 = b1.drain(1, 2);
///
/// print(b1);      // prints "[010405]"
///
/// print(b2);      // prints "[0203]"
///
/// let b3 = b1.drain(-1, 1);
///
/// print(b3);      // prints "[0104]"
///
/// print(z);       // prints "[5]"
/// ```
fn drain(blob: blob, start: int, len: int) -> blob;

/// Return the number of seconds between the current system time and the timestamp.
///
/// # Example
///
/// ```rhai
/// let now = timestamp();
///
/// sleep(10.0);            // sleep for 10 seconds
///
/// print(now.elapsed);     // prints 10.???
/// ```
fn elapsed(timestamp: timestamp) -> RhaiResult;

/// Return the end of the exclusive range.
fn end(range: range) -> int;

/// Return the end of the inclusive range.
fn end(range: range=) -> int;

/// Return `true` if the string ends with a specified character.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.ends_with('w'));         // prints true
///
/// print(text.ends_with('h'));         // prints false
/// ```
fn ends_with(string: string, character: char) -> bool;

/// Return `true` if the string ends with a specified string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.ends_with("world!"));    // prints true
///
/// print(text.ends_with("hello"));     // prints false
/// ```
fn ends_with(string: string, match_string: string) -> bool;

/// Exit the script evaluation immediately with `()` as exit value.
///
/// # Example
/// ```rhai
/// exit();
/// ```
fn exit() -> RhaiResult;

/// Exit the script evaluation immediately with a value.
///
/// # Example
/// ```rhai
/// exit(42);
/// ```
fn exit(value: ?) -> RhaiResult;

/// Return the exponential of the floating-point number.
fn exp(x: float) -> float;

/// Copy an exclusive range of the array and return it as a new array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// print(x.extract(1..3));     // prints "[2, 3]"
///
/// print(x);                   // prints "[1, 2, 3, 4, 5]"
/// ```
fn extract(array: array, range: range) -> array;

/// Copy an inclusive range of the array and return it as a new array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// print(x.extract(1..=3));    // prints "[2, 3, 4]"
///
/// print(x);                   // prints "[1, 2, 3, 4, 5]"
/// ```
fn extract(array: array, range: range=) -> array;

/// Copy a portion of the array beginning at the `start` position till the end and return it as
/// a new array.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, the entire array is copied and returned.
/// * If `start` ≥ length of array, an empty array is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// print(x.extract(2));        // prints "[3, 4, 5]"
///
/// print(x.extract(-3));       // prints "[3, 4, 5]"
///
/// print(x);                   // prints "[1, 2, 3, 4, 5]"
/// ```
fn extract(array: array, start: int) -> array;

/// Copy an exclusive `range` of the BLOB and return it as a new BLOB.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(b.extract(1..3));     // prints "[0203]"
///
/// print(b);                   // prints "[0102030405]"
/// ```
fn extract(blob: blob, range: range) -> blob;

/// Copy an inclusive `range` of the BLOB and return it as a new BLOB.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(b.extract(1..=3));    // prints "[020304]"
///
/// print(b);                   // prints "[0102030405]"
/// ```
fn extract(blob: blob, range: range=) -> blob;

/// Copy a portion of the BLOB beginning at the `start` position till the end and return it as
/// a new BLOB.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, the entire BLOB is copied and returned.
/// * If `start` ≥ length of BLOB, an empty BLOB is returned.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(b.extract(2));        // prints "[030405]"
///
/// print(b.extract(-3));       // prints "[030405]"
///
/// print(b);                   // prints "[0102030405]"
/// ```
fn extract(blob: blob, start: int) -> blob;

/// Copy a portion of the array and return it as a new array.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, an empty array is returned.
/// * If `len` ≤ 0, an empty array is returned.
/// * If `start` position + `len` ≥ length of array, entire portion of the array after the `start` position is copied and returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// print(x.extract(1, 3));     // prints "[2, 3, 4]"
///
/// print(x.extract(-3, 2));    // prints "[3, 4]"
///
/// print(x);                   // prints "[1, 2, 3, 4, 5]"
/// ```
fn extract(array: array, start: int, len: int) -> array;

/// Copy a portion of the BLOB and return it as a new BLOB.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, an empty BLOB is returned.
/// * If `len` ≤ 0, an empty BLOB is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is copied and returned.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(b.extract(1, 3));     // prints "[020303]"
///
/// print(b.extract(-3, 2));    // prints "[0304]"
///
/// print(b);                   // prints "[0102030405]"
/// ```
fn extract(blob: blob, start: int, len: int) -> blob;

/// Add all property values of another object map into the object map.
/// Only properties that do not originally exist in the object map are added.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
/// let n = #{a: 42, d:0};
///
/// m.fill_with(n);
///
/// print(m);       // prints "#{a:1, b:2, c:3, d:0}"
/// ```
fn fill_with(map: map, map2: map) -> ();

/// Iterate through all the elements in the array, applying a `filter` function to each element
/// in turn, and return a copy of all elements (in order) that return `true` as a new array.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `filter` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.filter(|v| v >= 3);
///
/// print(y);       // prints "[3, 4, 5]"
///
/// let y = x.filter(|v, i| v * i >= 10);
///
/// print(y);       // prints "[12, 20]"
/// ```
fn filter(array: array, filter: Fn) -> Array;

/// Iterate through all the elements in the array, applying a function named by `filter` to each
/// element in turn, and return a copy of all elements (in order) that return `true` as a new array.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.filter(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `filter` must exist taking these parameters:
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn screen(x, i) { x * i >= 10 }
///
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.filter("is_odd");
///
/// print(y);       // prints "[1, 3, 5]"
///
/// let y = x.filter("screen");
///
/// print(y);       // prints "[12, 20]"
/// ```
fn filter(array: array, filter_func: string) -> Array;

/// Iterate through all the elements in the object map, applying a `filter` function to each
/// and return a new collection of all elements that return `true` as a new object map.
///
/// # Function Parameters
///
/// * `key`: current key
/// * `value` _(optional)_: copy of element (bound to `this` if omitted)
///
/// # Example
///
/// ```rhai
/// let x = #{a:1, b:2, c:3, d:4, e:5};
///
/// let y = x.filter(|k| this >= 3);
///
/// print(y);       // prints #{"c":3, "d":4, "e":5}
///
/// let y = x.filter(|k, v| k != "d" && v < 5);
///
/// print(y);       // prints #{"a":1, "b":2, "c":3}
/// ```
fn filter(map: map, filter: Fn) -> Map;

/// Iterate through all the elements in the array, applying a `filter` function to each element
/// in turn, and return a copy of the first element that returns `true`. If no element returns
/// `true`, `()` is returned.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 5, 8, 13];
///
/// print(x.find(|v| v > 3));                    // prints 5: 5 > 3
///
/// print(x.find(|v| v > 13) ?? "not found");    // prints "not found": nothing is > 13
///
/// print(x.find(|v, i| v * i > 13));            // prints 5: 3 * 5 > 13
/// ```
fn find(array: array, filter: Fn) -> RhaiResult;

/// Iterate through all the elements in the array, starting from a particular `start` position,
/// applying a `filter` function to each element in turn, and return a copy of the first element
/// that returns `true`. If no element returns `true`, `()` is returned.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, `-1` is returned.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `filter` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 5, 8, 13];
///
/// print(x.find(|v| v > 1, 2));                     // prints 3: 3 > 1
///
/// print(x.find(|v| v < 2, 3) ?? "not found");      // prints "not found": nothing < 2 past index 3
///
/// print(x.find(|v| v > 1, 8) ?? "not found");      // prints "not found": nothing found past end of array
///
/// print(x.find(|v| v > 1, -3));                    // prints 5: -3 = start from index 4
///
/// print(x.find(|v| v > 0, -99));                   // prints 1: -99 = start from beginning
///
/// print(x.find(|v, i| v * i > 6, 3));              // prints 5: 5 * 4 > 6
/// ```
fn find(array: array, filter: Fn, start: int) -> RhaiResult;

/// Iterate through all the elements in the array, applying a `mapper` function to each element
/// in turn, and return the first result that is not `()`. Otherwise, `()` is returned.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `mapper` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [#{alice: 1}, #{bob: 2}, #{clara: 3}];
///
/// print(x.find_map(|v| v.alice));                  // prints 1
///
/// print(x.find_map(|v| v.dave) ?? "not found");    // prints "not found"
///
/// print(x.find_map(|| this.dave) ?? "not found");  // prints "not found"
/// ```
fn find_map(array: array, filter: Fn) -> RhaiResult;

/// Iterate through all the elements in the array, starting from a particular `start` position,
/// applying a `mapper` function to each element in turn, and return the first result that is not `()`.
/// Otherwise, `()` is returned.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, `-1` is returned.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `mapper` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [#{alice: 1}, #{bob: 2}, #{bob: 3}, #{clara: 3}, #{alice: 0}, #{clara: 5}];
///
/// print(x.find_map(|v| v.alice, 2));                   // prints 0
///
/// print(x.find_map(|v| v.bob, 4) ?? "not found");      // prints "not found"
///
/// print(x.find_map(|v| v.alice, 8) ?? "not found");    // prints "not found"
///
/// print(x.find_map(|| this.alice, 8) ?? "not found");  // prints "not found"
///
/// print(x.find_map(|v| v.bob, -4));                    // prints 3: -4 = start from index 2
///
/// print(x.find_map(|v| v.alice, -99));                 // prints 1: -99 = start from beginning
///
/// print(x.find_map(|| this.alice, -99));               // prints 1: -99 = start from beginning
/// ```
fn find_map(array: array, filter: Fn, start: int) -> RhaiResult;

/// Return the largest whole number less than or equals to the floating-point number.
fn floor(x: float) -> float;

/// Iterate through all the elements in the array, applying a `process` function to each element in turn.
/// Each element is bound to `this` before calling the function.
///
/// # Function Parameters
///
/// * `this`: bound to array element (mutable)
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// x.for_each(|| this *= this);
///
/// print(x);       // prints "[1, 4, 9, 16, 25]"
///
/// x.for_each(|i| this *= i);
///
/// print(x);       // prints "[0, 2, 6, 12, 20]"
/// ```
fn for_each(array: array, map: Fn) -> ();

/// Return the fractional part of the floating-point number.
fn fraction(x: float) -> float;

/// Get a copy of the element at the `index` position in the array.
///
/// * If `index` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `index` < -length of array, `()` is returned.
/// * If `index` ≥ length of array, `()` is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
///
/// print(x.get(0));        // prints 1
///
/// print(x.get(-1));       // prints 3
///
/// print(x.get(99));       // prints empty (for '()')
/// ```
fn get(array: array, index: int) -> ?;

/// Get the byte value at the `index` position in the BLOB.
///
/// * If `index` < 0, position counts from the end of the BLOB (`-1` is the last element).
/// * If `index` < -length of BLOB, zero is returned.
/// * If `index` ≥ length of BLOB, zero is returned.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(b.get(0));        // prints 1
///
/// print(b.get(-1));       // prints 5
///
/// print(b.get(99));       // prints 0
/// ```
fn get(blob: blob, index: int) -> int;

/// Get the value of the `property` in the object map and return a copy.
///
/// If `property` does not exist in the object map, `()` is returned.
///
/// # Example
///
/// ```rhai
/// let m = #{a: 1, b: 2, c: 3};
///
/// print(m.get("b"));      // prints 2
///
/// print(m.get("x"));      // prints empty (for '()')
/// ```
fn get(map: map, property: string) -> ?;

/// Get the character at the `index` position in the string.
///
/// * If `index` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `index` < -length of string, zero is returned.
/// * If `index` ≥ length of string, zero is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.get(0));     // prints 'h'
///
/// print(text.get(-1));    // prints '!'
///
/// print(text.get(99));    // prints empty (for '()')'
/// ```
fn get(string: string, index: int) -> ?;

/// Return an iterator over all the bits in the number.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// for bit in x.bits {
///     print(bit);
/// }
/// ```
fn get bits(value: int) -> BitRange;

/// Return the length of the string, in number of bytes used to store it in UTF-8 encoding.
///
/// # Example
///
/// ```rhai
/// let text = "朝には紅顔ありて夕べには白骨となる";
///
/// print(text.bytes);      // prints 51
/// ```
fn get bytes(string: string) -> int;

/// Return the smallest whole number larger than or equals to the floating-point number.
fn get ceiling(x: float) -> float;

/// Return an iterator over all the characters in the string.
///
/// # Example
///
/// ```rhai
/// for ch in "hello, world!".chars {"
///     print(ch);
/// }
/// ```
fn get chars(string: string) -> CharsStream;

/// Return the number of seconds between the current system time and the timestamp.
///
/// # Example
///
/// ```rhai
/// let now = timestamp();
///
/// sleep(10.0);            // sleep for 10 seconds
///
/// print(now.elapsed);     // prints 10.???
/// ```
fn get elapsed(timestamp: timestamp) -> RhaiResult;

/// Return the end of the exclusive range.
fn get end(range: range) -> int;

/// Return the end of the inclusive range.
fn get end(range: range=) -> int;

/// Return the largest whole number less than or equals to the floating-point number.
fn get floor(x: float) -> float;

/// Return the fractional part of the floating-point number.
fn get fraction(x: float) -> float;

/// Return the integral part of the floating-point number.
fn get int(x: float) -> float;

/// Return `true` if the function is an anonymous function.
///
/// # Example
///
/// ```rhai
/// let f = |x| x * 2;
///
/// print(f.is_anonymous);      // prints true
/// ```
fn get is_anonymous(fn_ptr: Fn) -> bool;

/// Return true if the array is empty.
fn get is_empty(array: array) -> bool;

/// Return true if the BLOB is empty.
fn get is_empty(blob: blob) -> bool;

/// Return true if the range contains no items.
fn get is_empty(range: range) -> bool;

/// Return true if the range contains no items.
fn get is_empty(range: range=) -> bool;

/// Return true if the string is empty.
fn get is_empty(string: string) -> bool;

/// Return true if the number is even.
fn get is_even(x: int) -> bool;

/// Return true if the number is even.
fn get is_even(x: i128) -> bool;

/// Return true if the number is even.
fn get is_even(x: i16) -> bool;

/// Return true if the number is even.
fn get is_even(x: i32) -> bool;

/// Return true if the number is even.
fn get is_even(x: i8) -> bool;

/// Return true if the number is even.
fn get is_even(x: u128) -> bool;

/// Return true if the number is even.
fn get is_even(x: u16) -> bool;

/// Return true if the number is even.
fn get is_even(x: u32) -> bool;

/// Return true if the number is even.
fn get is_even(x: u64) -> bool;

/// Return true if the number is even.
fn get is_even(x: u8) -> bool;

/// Return `true` if the range is exclusive.
fn get is_exclusive(range: range) -> bool;

/// Return `true` if the range is exclusive.
fn get is_exclusive(range: range=) -> bool;

/// Return `true` if the floating-point number is finite.
fn get is_finite(x: float) -> bool;

/// Return `true` if the range is inclusive.
fn get is_inclusive(range: range) -> bool;

/// Return `true` if the range is inclusive.
fn get is_inclusive(range: range=) -> bool;

/// Return `true` if the floating-point number is infinite.
fn get is_infinite(x: float) -> bool;

/// Return `true` if the floating-point number is `NaN` (Not A Number).
fn get is_nan(x: float) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: int) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: i128) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: i16) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: i32) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: i8) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: u128) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: u16) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: u32) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: u64) -> bool;

/// Return true if the number is odd.
fn get is_odd(x: u8) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: int) -> bool;

/// Return true if the floating-point number is zero.
fn get is_zero(x: f32) -> bool;

/// Return true if the floating-point number is zero.
fn get is_zero(x: float) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: i128) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: i16) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: i32) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: i8) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: u128) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: u16) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: u32) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: u64) -> bool;

/// Return true if the number is zero.
fn get is_zero(x: u8) -> bool;

/// Number of elements in the array.
fn get len(array: array) -> int;

/// Return the length of the BLOB.
///
/// # Example
///
/// ```rhai
/// let b = blob(10, 0x42);
///
/// print(b);           // prints "[4242424242424242 4242]"
///
/// print(b.len());     // prints 10
/// ```
fn get len(blob: blob) -> int;

/// Return the length of the string, in number of characters.
///
/// # Example
///
/// ```rhai
/// let text = "朝には紅顔ありて夕べには白骨となる";
///
/// print(text.len);        // prints 17
/// ```
fn get len(string: string) -> int;

/// Return the name of the function.
///
/// # Example
///
/// ```rhai
/// fn double(x) { x * 2 }
///
/// let f = Fn("double");
///
/// print(f.name);      // prints "double"
/// ```
fn get name(fn_ptr: Fn) -> string;

/// Return the nearest whole number closest to the floating-point number.
/// Rounds away from zero.
fn get round(x: float) -> float;

/// Return the start of the exclusive range.
fn get start(range: range) -> int;

/// Return the start of the inclusive range.
fn get start(range: range=) -> int;

/// Return the _tag_ of a `Dynamic` value.
///
/// # Example
///
/// ```rhai
/// let x = "hello, world!";
///
/// x.tag = 42;
///
/// print(x.tag);           // prints 42
/// ```
fn get tag(value: ?) -> int;

/// Return `true` if the specified `bit` in the number is set.
///
/// If `bit` < 0, position counts from the MSB (Most Significant Bit).
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// print(x.get_bit(5));    // prints false
///
/// print(x.get_bit(6));    // prints true
///
/// print(x.get_bit(-48));  // prints true on 64-bit
/// ```
fn get_bit(value: int, bit: int) -> bool;

/// Return an exclusive range of bits in the number as a new number.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// print(x.get_bits(5..10));       // print 18
/// ```
fn get_bits(value: int, range: range) -> int;

/// Return an inclusive range of bits in the number as a new number.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// print(x.get_bits(5..=9));       // print 18
/// ```
fn get_bits(value: int, range: range=) -> int;

/// Return a portion of bits in the number as a new number.
///
/// * If `start` < 0, position counts from the MSB (Most Significant Bit).
/// * If `bits` ≤ 0, zero is returned.
/// * If `start` position + `bits` ≥ total number of bits, the bits after the `start` position are returned.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// print(x.get_bits(5, 8));        // print 18
/// ```
fn get_bits(value: int, start: int, bits: int) -> int;

/// Return an array of object maps containing metadata of all script-defined functions.
fn get_fn_metadata_list() -> array;

/// Return an array of object maps containing metadata of all script-defined functions
/// matching the specified name.
fn get_fn_metadata_list(name: string) -> array;

/// Return an array of object maps containing metadata of all script-defined functions
/// matching the specified name and arity (number of parameters).
fn get_fn_metadata_list(name: string, params: int) -> array;

/// Return the hypotenuse of a triangle with sides `x` and `y`.
fn hypot(x: float, y: float) -> float;

/// Iterate through all the elements in the array, applying a function named by `filter` to each
/// element in turn, and return the index of the first element that returns `true`.
/// If no element returns `true`, `-1` is returned.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.index_of(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `filter` must exist taking these parameters:
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn is_special(x) { x > 3 }
///
/// fn is_dumb(x) { x > 8 }
///
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.index_of("is_special"));    // prints 3
///
/// print(x.index_of("is_dumb"));       // prints -1
/// ```
fn index_of(array: array, filter: string) -> int;

/// Iterate through all the elements in the array, applying a `filter` function to each element
/// in turn, and return the index of the first element that returns `true`.
/// If no element returns `true`, `-1` is returned.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `filter` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.index_of(|v| v > 3));           // prints 3: 4 > 3
///
/// print(x.index_of(|v| v > 8));           // prints -1: nothing is > 8
///
/// print(x.index_of(|v, i| v * i > 20));   // prints 7: 4 * 7 > 20
/// ```
fn index_of(array: array, filter: Fn) -> int;

/// Find the first element in the array that equals a particular `value` and return its index.
/// If no element equals `value`, `-1` is returned.
///
/// The operator `==` is used to compare elements with `value` and must be defined,
/// otherwise `false` is assumed.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.index_of(4));       // prints 3 (first index)
///
/// print(x.index_of(9));       // prints -1
///
/// print(x.index_of("foo"));   // prints -1: strings do not equal numbers
/// ```
fn index_of(array: array, value: ?) -> int;

/// Find the specified `character` in the string and return the first index where it is found.
/// If the `character` is not found, `-1` is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.index_of('l'));      // prints 2 (first index)
///
/// print(text.index_of('x'));      // prints -1
/// ```
fn index_of(string: string, character: char) -> int;

/// Find the specified `character` in the string and return the first index where it is found.
/// If the `character` is not found, `-1` is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// print(text.index_of("ll"));     // prints 2 (first index)
///
/// print(text.index_of("xx:));     // prints -1
/// ```
fn index_of(string: string, find_string: string) -> int;

/// Iterate through all the elements in the array, starting from a particular `start` position,
/// applying a function named by `filter` to each element in turn, and return the index of the
/// first element that returns `true`. If no element returns `true`, `-1` is returned.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, `-1` is returned.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.index_of(Fn("fn_name"), start)` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `filter` must exist taking these parameters:
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn plural(x) { x > 1 }
///
/// fn singular(x) { x < 2 }
///
/// fn screen(x, i) { x * i > 20 }
///
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.index_of("plural", 3));     // prints 5: 2 > 1
///
/// print(x.index_of("singular", 9));   // prints -1: nothing < 2 past index 9
///
/// print(x.index_of("plural", 15));    // prints -1: nothing found past end of array
///
/// print(x.index_of("plural", -5));    // prints 9: -5 = start from index 8
///
/// print(x.index_of("plural", -99));   // prints 1: -99 = start from beginning
///
/// print(x.index_of("screen", 8));     // prints 10: 3 * 10 > 20
/// ```
fn index_of(array: array, filter: string, start: int) -> int;

/// Iterate through all the elements in the array, starting from a particular `start` position,
/// applying a `filter` function to each element in turn, and return the index of the first
/// element that returns `true`. If no element returns `true`, `-1` is returned.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, `-1` is returned.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `filter` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.index_of(|v| v > 1, 3));    // prints 5: 2 > 1
///
/// print(x.index_of(|v| v < 2, 9));    // prints -1: nothing < 2 past index 9
///
/// print(x.index_of(|v| v > 1, 15));   // prints -1: nothing found past end of array
///
/// print(x.index_of(|v| v > 1, -5));   // prints 9: -5 = start from index 8
///
/// print(x.index_of(|v| v > 1, -99));  // prints 1: -99 = start from beginning
///
/// print(x.index_of(|v, i| v * i > 20, 8));    // prints 10: 3 * 10 > 20
/// ```
fn index_of(array: array, filter: Fn, start: int) -> int;

/// Find the first element in the array, starting from a particular `start` position, that
/// equals a particular `value` and return its index. If no element equals `value`, `-1` is returned.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, `-1` is returned.
///
/// The operator `==` is used to compare elements with `value` and must be defined,
/// otherwise `false` is assumed.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.index_of(4, 2));        // prints 3
///
/// print(x.index_of(4, 5));        // prints 7
///
/// print(x.index_of(4, 15));       // prints -1: nothing found past end of array
///
/// print(x.index_of(4, -5));       // prints 11: -5 = start from index 8
///
/// print(x.index_of(9, 1));        // prints -1: nothing equals 9
///
/// print(x.index_of("foo", 1));    // prints -1: strings do not equal numbers
/// ```
fn index_of(array: array, value: ?, start: int) -> int;

/// Find the specified `character` in the string, starting from the specified `start` position,
/// and return the first index where it is found.
/// If the `character` is not found, `-1` is returned.
///
/// * If `start` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `start` < -length of string, position counts from the beginning of the string.
/// * If `start` ≥ length of string, `-1` is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.index_of('l', 5));       // prints 10 (first index after 5)
///
/// print(text.index_of('o', -7));      // prints 8
///
/// print(text.index_of('x', 0));       // prints -1
/// ```
fn index_of(string: string, character: char, start: int) -> int;

/// Find the specified sub-string in the string, starting from the specified `start` position,
/// and return the first index where it is found.
/// If the sub-string is not found, `-1` is returned.
///
/// * If `start` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `start` < -length of string, position counts from the beginning of the string.
/// * If `start` ≥ length of string, `-1` is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// print(text.index_of("ll", 5));      // prints 16 (first index after 5)
///
/// print(text.index_of("ll", -15));    // prints 16
///
/// print(text.index_of("xx", 0));      // prints -1
/// ```
fn index_of(string: string, find_string: string, start: int) -> int;

/// Add a new element into the array at a particular `index` position.
///
/// * If `index` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `index` < -length of array, the element is added to the beginning of the array.
/// * If `index` ≥ length of array, the element is appended to the end of the array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
///
/// x.insert(0, "hello");
///
/// x.insert(2, true);
///
/// x.insert(-2, 42);
///
/// print(x);       // prints ["hello", 1, true, 2, 42, 3]
/// ```
fn insert(array: array, index: int, item: ?) -> ();

/// Add a byte `value` to the BLOB at a particular `index` position.
///
/// * If `index` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `index` < -length of BLOB, the byte value is added to the beginning of the BLOB.
/// * If `index` ≥ length of BLOB, the byte value is appended to the end of the BLOB.
///
/// Only the lower 8 bits of the `value` are used; all other bits are ignored.
///
/// # Example
///
/// ```rhai
/// let b = blob(5, 0x42);
///
/// b.insert(2, 0x18);
///
/// print(b);       // prints "[4242184242]"
/// ```
fn insert(blob: blob, index: int, value: int) -> ();

/// Return the integral part of the floating-point number.
fn int(x: float) -> float;

/// Return `true` if the function is an anonymous function.
///
/// # Example
///
/// ```rhai
/// let f = |x| x * 2;
///
/// print(f.is_anonymous);      // prints true
/// ```
fn is_anonymous(fn_ptr: Fn) -> bool;

/// Return true if the array is empty.
fn is_empty(array: array) -> bool;

/// Return true if the BLOB is empty.
fn is_empty(blob: blob) -> bool;

/// Return true if the map is empty.
fn is_empty(map: map) -> bool;

/// Return true if the range contains no items.
fn is_empty(range: range) -> bool;

/// Return true if the range contains no items.
fn is_empty(range: range=) -> bool;

/// Return true if the string is empty.
fn is_empty(string: string) -> bool;

/// Return true if the number is even.
fn is_even(x: int) -> bool;

/// Return true if the number is even.
fn is_even(x: i128) -> bool;

/// Return true if the number is even.
fn is_even(x: i16) -> bool;

/// Return true if the number is even.
fn is_even(x: i32) -> bool;

/// Return true if the number is even.
fn is_even(x: i8) -> bool;

/// Return true if the number is even.
fn is_even(x: u128) -> bool;

/// Return true if the number is even.
fn is_even(x: u16) -> bool;

/// Return true if the number is even.
fn is_even(x: u32) -> bool;

/// Return true if the number is even.
fn is_even(x: u64) -> bool;

/// Return true if the number is even.
fn is_even(x: u8) -> bool;

/// Return `true` if the range is exclusive.
fn is_exclusive(range: range) -> bool;

/// Return `true` if the range is exclusive.
fn is_exclusive(range: range=) -> bool;

/// Return `true` if the floating-point number is finite.
fn is_finite(x: float) -> bool;

/// Return `true` if the range is inclusive.
fn is_inclusive(range: range) -> bool;

/// Return `true` if the range is inclusive.
fn is_inclusive(range: range=) -> bool;

/// Return `true` if the floating-point number is infinite.
fn is_infinite(x: float) -> bool;

/// Return `true` if the floating-point number is `NaN` (Not A Number).
fn is_nan(x: float) -> bool;

/// Return true if the number is odd.
fn is_odd(x: int) -> bool;

/// Return true if the number is odd.
fn is_odd(x: i128) -> bool;

/// Return true if the number is odd.
fn is_odd(x: i16) -> bool;

/// Return true if the number is odd.
fn is_odd(x: i32) -> bool;

/// Return true if the number is odd.
fn is_odd(x: i8) -> bool;

/// Return true if the number is odd.
fn is_odd(x: u128) -> bool;

/// Return true if the number is odd.
fn is_odd(x: u16) -> bool;

/// Return true if the number is odd.
fn is_odd(x: u32) -> bool;

/// Return true if the number is odd.
fn is_odd(x: u64) -> bool;

/// Return true if the number is odd.
fn is_odd(x: u8) -> bool;

/// Return true if the number is zero.
fn is_zero(x: int) -> bool;

/// Return true if the floating-point number is zero.
fn is_zero(x: f32) -> bool;

/// Return true if the floating-point number is zero.
fn is_zero(x: float) -> bool;

/// Return true if the number is zero.
fn is_zero(x: i128) -> bool;

/// Return true if the number is zero.
fn is_zero(x: i16) -> bool;

/// Return true if the number is zero.
fn is_zero(x: i32) -> bool;

/// Return true if the number is zero.
fn is_zero(x: i8) -> bool;

/// Return true if the number is zero.
fn is_zero(x: u128) -> bool;

/// Return true if the number is zero.
fn is_zero(x: u16) -> bool;

/// Return true if the number is zero.
fn is_zero(x: u32) -> bool;

/// Return true if the number is zero.
fn is_zero(x: u64) -> bool;

/// Return true if the number is zero.
fn is_zero(x: u8) -> bool;

/// Return an array with all the property names in the object map.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
///
/// print(m.keys());        // prints ["a", "b", "c"]
/// ```
fn keys(map: map) -> array;

/// Number of elements in the array.
fn len(array: array) -> int;

/// Return the length of the BLOB.
///
/// # Example
///
/// ```rhai
/// let b = blob(10, 0x42);
///
/// print(b);           // prints "[4242424242424242 4242]"
///
/// print(b.len());     // prints 10
/// ```
fn len(blob: blob) -> int;

/// Return the number of properties in the object map.
fn len(map: map) -> int;

/// Return the length of the string, in number of characters.
///
/// # Example
///
/// ```rhai
/// let text = "朝には紅顔ありて夕べには白骨となる";
///
/// print(text.len);        // prints 17
/// ```
fn len(string: string) -> int;

/// Return the natural log of the floating-point number.
fn ln(x: float) -> float;

/// Return the log of the floating-point number with base 10.
fn log(x: float) -> float;

/// Return the log of the floating-point number with `base`.
fn log(x: float, base: float) -> float;

/// Convert the character to lower-case.
///
/// # Example
///
/// ```rhai
/// let ch = 'A';
///
/// ch.make_lower();
///
/// print(ch);          // prints 'a'
/// ```
fn make_lower(character: char) -> ();

/// Convert the string to all lower-case.
///
/// # Example
///
/// ```rhai
/// let text = "HELLO, WORLD!"
///
/// text.make_lower();
///
/// print(text);        // prints "hello, world!";
/// ```
fn make_lower(string: string) -> ();

/// Convert the character to upper-case.
///
/// # Example
///
/// ```rhai
/// let ch = 'a';
///
/// ch.make_upper();
///
/// print(ch);          // prints 'A'
/// ```
fn make_upper(character: char) -> ();

/// Convert the string to all upper-case.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!"
///
/// text.make_upper();
///
/// print(text);        // prints "HELLO, WORLD!";
/// ```
fn make_upper(string: string) -> ();

/// Iterate through all the elements in the array, applying a `mapper` function to each element
/// in turn, and return the results as a new array.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `mapper` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.map(|v| v * v);
///
/// print(y);       // prints "[1, 4, 9, 16, 25]"
///
/// let y = x.map(|v, i| v * i);
///
/// print(y);       // prints "[0, 2, 6, 12, 20]"
/// ```
fn map(array: array, map: Fn) -> Array;

/// Iterate through all the elements in the array, applying a function named by `mapper` to each
/// element in turn, and return the results as a new array.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.map(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `mapper` must exist taking these parameters:
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn square(x) { x * x }
///
/// fn multiply(x, i) { x * i }
///
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.map("square");
///
/// print(y);       // prints "[1, 4, 9, 16, 25]"
///
/// let y = x.map("multiply");
///
/// print(y);       // prints "[0, 2, 6, 12, 20]"
/// ```
fn map(array: array, mapper: string) -> Array;

/// Iterate through all the elements in the object map, applying a `mapper` function to each
/// element in turn, and return the results as a new object map.
///
/// # Function Parameters
///
/// * `key`: current key
/// * `value` _(optional)_: copy of element (bound to `this` if omitted)
///
/// # Example
///
/// ```rhai
/// let x = #{a:1, b:2, c:3, d:4, e:5};
///
/// let y = x.map(|k| this * 2);
///
/// print(y);       // prints #{"a":2, "b":4, "c":6, "d":8, "e":10}
///
/// let y = x.filter(|k, v| v + k.len());
///
/// print(y);       // prints #{"a":2, "b":3, "c":5, "d":6, "e":7}
/// ```
fn map(map: map, mapper: Fn) -> Map;

/// Return the character that is lexically greater than the other character.
///
/// # Example
///
/// ```rhai
/// max('h', 'w');      // returns 'w'
/// ```
fn max(char1: char, char2: char) -> char;

/// Return the string that is lexically greater than the other string.
///
/// # Example
///
/// ```rhai
/// max("hello", "world");      // returns "world"
/// ```
fn max(string1: string, string2: string) -> string;

fn max(x: float, y: f32) -> float;

/// Return the number that is larger than the other number.
///
/// # Example
///
/// ```rhai
/// max(42, 123);   // returns 132
/// ```
fn max(x: int, y: int) -> int;

fn max(x: int, y: f32) -> f32;

fn max(x: int, y: float) -> float;

fn max(x: f32, y: float) -> float;

fn max(x: f32, y: int) -> f32;

fn max(x: f32, y: f32) -> f32;

fn max(x: float, y: int) -> float;

fn max(x: float, y: float) -> float;

fn max(x: i128, y: i128) -> i128;

fn max(x: i16, y: i16) -> i16;

fn max(x: i32, y: i32) -> i32;

fn max(x: i8, y: i8) -> i8;

fn max(x: u128, y: u128) -> u128;

fn max(x: u16, y: u16) -> u16;

fn max(x: u32, y: u32) -> u32;

fn max(x: u64, y: u64) -> u64;

fn max(x: u8, y: u8) -> u8;

/// Return the character that is lexically smaller than the other character.
///
/// # Example
///
/// ```rhai
/// max('h', 'w');      // returns 'h'
/// ```
fn min(char1: char, char2: char) -> char;

/// Return the string that is lexically smaller than the other string.
///
/// # Example
///
/// ```rhai
/// min("hello", "world");      // returns "hello"
/// ```
fn min(string1: string, string2: string) -> string;

fn min(x: float, y: f32) -> float;

/// Return the number that is smaller than the other number.
///
/// # Example
///
/// ```rhai
/// min(42, 123);   // returns 42
/// ```
fn min(x: int, y: int) -> int;

fn min(x: int, y: f32) -> f32;

fn min(x: int, y: float) -> float;

fn min(x: f32, y: float) -> float;

fn min(x: f32, y: int) -> f32;

fn min(x: f32, y: f32) -> f32;

fn min(x: float, y: int) -> float;

fn min(x: float, y: float) -> float;

fn min(x: i128, y: i128) -> i128;

fn min(x: i16, y: i16) -> i16;

fn min(x: i32, y: i32) -> i32;

fn min(x: i8, y: i8) -> i8;

fn min(x: u128, y: u128) -> u128;

fn min(x: u16, y: u16) -> u16;

fn min(x: u32, y: u32) -> u32;

fn min(x: u64, y: u64) -> u64;

fn min(x: u8, y: u8) -> u8;

/// Add all property values of another object map into the object map.
/// Existing property values of the same names are replaced.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
/// let n = #{a: 42, d:0};
///
/// m.mixin(n);
///
/// print(m);       // prints "#{a:42, b:2, c:3, d:0}"
/// ```
fn mixin(map: map, map2: map) -> ();

/// Return the name of the function.
///
/// # Example
///
/// ```rhai
/// fn double(x) { x * 2 }
///
/// let f = Fn("double");
///
/// print(f.name);      // prints "double"
/// ```
fn name(fn_ptr: Fn) -> string;

/// Sort the array and return it as a new array.
///
/// All elements in the array must be of the same data type.
///
/// # Supported Data Types
///
/// * integer numbers
/// * floating-point numbers
/// * decimal numbers
/// * characters
/// * strings
/// * booleans
/// * `()`
///
/// # Example
///
/// ```rhai
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// let y = x.order();
///
/// print(y);       // prints "[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]"
/// ```
fn order(array: array) -> Array;

/// Sort the array based on applying the `comparer` function and return it as a new array.
///
/// The `comparer` function must implement a [total order](https://en.wikipedia.org/wiki/Total_order).
///
/// # Function Parameters
///
/// * `element1`: copy of the current array element to compare
/// * `element2`: copy of the next array element to compare
///
/// ## Return Value
///
/// An integer number:
///
/// * Any positive integer if `element1 > element2`
/// * 0 if `element1 == element2`
/// * Any negative integer if `element1 < element2`
///
/// or a boolean value:
///
/// * `true` if `element1 <= element2`
/// * `false` if `element1 > element2`
///
/// Any other return value type will yield unpredictable order.
///
/// # Errors
///
/// If the `comparer` function does not implement a [total order](https://en.wikipedia.org/wiki/Total_order),
/// an error is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// // Do comparisons in reverse
/// let y = x.order_by(|a, b| if a > b { -1 } else if a < b { 1 } else { 0 });
///
/// print(y);       // prints "[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]"
/// ```
fn order(array: array, comparer: Fn) -> Array;

/// Sort the array based on applying the `comparer` function and return it as a new array.
///
/// The `comparer` function must implement a [total order](https://en.wikipedia.org/wiki/Total_order).
///
/// # Function Parameters
///
/// * `element1`: copy of the current array element to compare
/// * `element2`: copy of the next array element to compare
///
/// ## Return Value
///
/// An integer number:
///
/// * Any positive integer if `element1 > element2`
/// * 0 if `element1 == element2`
/// * Any negative integer if `element1 < element2`
///
/// or a boolean value:
///
/// * `true` if `element1 <= element2`
/// * `false` if `element1 > element2`
///
/// Any other return value type will yield unpredictable order.
///
/// # Errors
///
/// If the `comparer` function does not implement a [total order](https://en.wikipedia.org/wiki/Total_order),
/// an error is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// // Do comparisons in reverse
/// let y = x.order_by(|a, b| if a > b { -1 } else if a < b { 1 } else { 0 });
///
/// print(y);       // prints "[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]"
/// ```
fn order_by(array: array, comparer: Fn) -> Array;

/// Sort the array in descending order and return it as a new array.
///
/// All elements in the array must be of the same data type.
///
/// # Supported Data Types
///
/// * integer numbers
/// * floating-point numbers
/// * decimal numbers
/// * characters
/// * strings
/// * booleans
/// * `()`
///
/// # Example
///
/// ```rhai
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// let y = x.order_desc();
///
/// print(y);       // prints "[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]"
/// ```
fn order_desc(array: array) -> Array;

/// Pad the array to at least the specified length with copies of a specified element.
///
/// If `len` ≤ length of array, no padding is done.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
///
/// x.pad(5, 42);
///
/// print(x);       // prints "[1, 2, 3, 42, 42]"
///
/// x.pad(3, 123);
///
/// print(x);       // prints "[1, 2, 3, 42, 42]"
/// ```
fn pad(array: array, len: int, item: ?) -> ();

/// Pad the BLOB to at least the specified length with copies of a specified byte `value`.
///
/// If `len` ≤ length of BLOB, no padding is done.
///
/// Only the lower 8 bits of the `value` are used; all other bits are ignored.
///
/// # Example
///
/// ```rhai
/// let b = blob(3, 0x42);
///
/// b.pad(5, 0x18)
///
/// print(b);               // prints "[4242421818]"
///
/// b.pad(3, 0xab)
///
/// print(b);               // prints "[4242421818]"
/// ```
fn pad(blob: blob, len: int, value: int) -> ();

/// Pad the string to at least the specified number of characters with the specified `character`.
///
/// If `len` ≤ length of string, no padding is done.
///
/// # Example
///
/// ```rhai
/// let text = "hello";
///
/// text.pad(8, '!');
///
/// print(text);        // prints "hello!!!"
///
/// text.pad(5, '*');
///
/// print(text);        // prints "hello!!!"
/// ```
fn pad(string: string, len: int, character: char) -> ();

/// Pad the string to at least the specified number of characters with the specified string.
///
/// If `len` ≤ length of string, no padding is done.
///
/// # Example
///
/// ```rhai
/// let text = "hello";
///
/// text.pad(10, "(!)");
///
/// print(text);        // prints "hello(!)(!)"
///
/// text.pad(8, '***');
///
/// print(text);        // prints "hello(!)(!)"
/// ```
fn pad(string: string, len: int, padding: string) -> ();

/// Parse the bytes within an exclusive `range` in the BLOB as a `FLOAT`
/// in big-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, zeros are padded.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes are ignored.
fn parse_be_float(blob: blob, range: range) -> float;

/// Parse the bytes within an inclusive `range` in the BLOB as a `FLOAT`
/// in big-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, zeros are padded.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes are ignored.
fn parse_be_float(blob: blob, range: range=) -> float;

/// Parse the bytes beginning at the `start` position in the BLOB as a `FLOAT`
/// in big-endian byte order.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, zero is returned.
/// * If `len` ≤ 0, zero is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is parsed.
///
/// * If number of bytes in range < number of bytes for `FLOAT`, zeros are padded.
/// * If number of bytes in range > number of bytes for `FLOAT`, extra bytes are ignored.
fn parse_be_float(blob: blob, start: int, len: int) -> float;

/// Parse the bytes within an exclusive `range` in the BLOB as an `INT`
/// in big-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `INT`, zeros are padded.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes are ignored.
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// let x = b.parse_be_int(1..3);   // parse two bytes
///
/// print(x.to_hex());              // prints "02030000...00"
/// ```
fn parse_be_int(blob: blob, range: range) -> int;

/// Parse the bytes within an inclusive `range` in the BLOB as an `INT`
/// in big-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `INT`, zeros are padded.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes are ignored.
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// let x = b.parse_be_int(1..=3);  // parse three bytes
///
/// print(x.to_hex());              // prints "0203040000...00"
/// ```
fn parse_be_int(blob: blob, range: range=) -> int;

/// Parse the bytes beginning at the `start` position in the BLOB as an `INT`
/// in big-endian byte order.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, zero is returned.
/// * If `len` ≤ 0, zero is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is parsed.
///
/// * If number of bytes in range < number of bytes for `INT`, zeros are padded.
/// * If number of bytes in range > number of bytes for `INT`, extra bytes are ignored.
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// let x = b.parse_be_int(1, 2);
///
/// print(x.to_hex());      // prints "02030000...00"
/// ```
fn parse_be_int(blob: blob, start: int, len: int) -> int;

/// Parse a string into a floating-point number.
///
/// # Example
///
/// ```rhai
/// let x = parse_int("123.456");
///
/// print(x);       // prints 123.456
/// ```
fn parse_float(string: string) -> float;

/// Parse a string into an integer number.
///
/// # Example
///
/// ```rhai
/// let x = parse_int("123");
///
/// print(x);       // prints 123
/// ```
fn parse_int(string: string) -> int;

/// Parse a string into an integer number of the specified `radix`.
///
/// `radix` must be between 2 and 36.
///
/// # Example
///
/// ```rhai
/// let x = parse_int("123");
///
/// print(x);       // prints 123
///
/// let y = parse_int("123abc", 16);
///
/// print(y);       // prints 1194684 (0x123abc)
/// ```
fn parse_int(string: string, radix: int) -> int;

/// Parse a JSON string into a value.
///
/// # Example
///
/// ```rhai
/// let m = parse_json(`{"a":1, "b":2, "c":3}`);
///
/// print(m);       // prints #{"a":1, "b":2, "c":3}
/// ```
fn parse_json(json: string) -> ?;

/// Parse the bytes within an exclusive `range` in the BLOB as a `FLOAT`
/// in little-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, zeros are padded.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes are ignored.
fn parse_le_float(blob: blob, range: range) -> float;

/// Parse the bytes within an inclusive `range` in the BLOB as a `FLOAT`
/// in little-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, zeros are padded.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes are ignored.
fn parse_le_float(blob: blob, range: range=) -> float;

/// Parse the bytes beginning at the `start` position in the BLOB as a `FLOAT`
/// in little-endian byte order.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, zero is returned.
/// * If `len` ≤ 0, zero is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is parsed.
///
/// * If number of bytes in range < number of bytes for `FLOAT`, zeros are padded.
/// * If number of bytes in range > number of bytes for `FLOAT`, extra bytes are ignored.
fn parse_le_float(blob: blob, start: int, len: int) -> float;

/// Parse the bytes within an exclusive `range` in the BLOB as an `INT`
/// in little-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `INT`, zeros are padded.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes are ignored.
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// let x = b.parse_le_int(1..3);   // parse two bytes
///
/// print(x.to_hex());              // prints "0302"
/// ```
fn parse_le_int(blob: blob, range: range) -> int;

/// Parse the bytes within an inclusive `range` in the BLOB as an `INT`
/// in little-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `INT`, zeros are padded.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes are ignored.
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// let x = b.parse_le_int(1..=3);  // parse three bytes
///
/// print(x.to_hex());              // prints "040302"
/// ```
fn parse_le_int(blob: blob, range: range=) -> int;

/// Parse the bytes beginning at the `start` position in the BLOB as an `INT`
/// in little-endian byte order.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, zero is returned.
/// * If `len` ≤ 0, zero is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is parsed.
///
/// * If number of bytes in range < number of bytes for `INT`, zeros are padded.
/// * If number of bytes in range > number of bytes for `INT`, extra bytes are ignored.
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// let x = b.parse_le_int(1, 2);
///
/// print(x.to_hex());      // prints "0302"
/// ```
fn parse_le_int(blob: blob, start: int, len: int) -> int;

/// Remove the last element from the array and return it.
///
/// If the array is empty, `()` is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
///
/// print(x.pop());     // prints 3
///
/// print(x);           // prints "[1, 2]"
/// ```
fn pop(array: array) -> ?;

/// Remove the last byte from the BLOB and return it.
///
/// If the BLOB is empty, zero is returned.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(b.pop());         // prints 5
///
/// print(b);               // prints "[01020304]"
/// ```
fn pop(blob: blob) -> int;

/// Remove the last character from the string and return it.
///
/// If the string is empty, `()` is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.pop());      // prints '!'
///
/// print(text);            // prints "hello, world"
/// ```
fn pop(string: string) -> ?;

/// Remove a specified number of characters from the end of the string and return it as a
/// new string.
///
/// * If `len` ≤ 0, the string is not modified and an empty string is returned.
/// * If `len` ≥ length of string, the string is cleared and the entire string returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.pop(4));     // prints "rld!"
///
/// print(text);            // prints "hello, wo"
/// ```
fn pop(string: string, len: int) -> string;

/// Return the empty string.
op print() -> string;

/// Convert the array into a string.
op print(array) -> string;

/// Return the character into a string.
op print(char) -> string;

/// Convert the value of the `item` into a string.
op print(?) -> string;

/// Convert the object map into a string.
op print(map) -> string;

/// Convert the value of `number` into a string.
op print(f32) -> string;

/// Convert the value of `number` into a string.
op print(float) -> string;

/// Return the `string`.
op print(string) -> string;

/// Return the empty string.
op print(()) -> string;

/// Return the boolean value into a string.
op print(bool) -> string;

/// Add a new element, which is not another array, to the end of the array.
///
/// If `item` is `Array`, then `append` is more specific and will be called instead.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
///
/// x.push("hello");
///
/// print(x);       // prints [1, 2, 3, "hello"]
/// ```
fn push(array: array, item: ?) -> ();

/// Add a new byte `value` to the end of the BLOB.
///
/// Only the lower 8 bits of the `value` are used; all other bits are ignored.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b.push(0x42);
///
/// print(b);       // prints "[42]"
/// ```
fn push(blob: blob, value: int) -> ();

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: i128, to: i128) -> Range<i128 >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: i16, to: i16) -> Range<i16 >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: i32, to: i32) -> Range<i32 >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: int, to: int) -> Range<int >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: i8, to: i8) -> Range<i8 >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: u128, to: u128) -> Range<u128 >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: u16, to: u16) -> Range<u16 >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: u32, to: u32) -> Range<u32 >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: u64, to: u64) -> Range<u64 >;

/// Return an iterator over the exclusive range of `from..to`.
/// The value `to` is never included.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17
/// for n in range(8, 18) {
///     print(n);
/// }
/// ```
fn range(from: u8, to: u8) -> Range<u8 >;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<float >, step: float) -> StepRange<float>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<i128 >, step: i128) -> StepRange<i128>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<i16 >, step: i16) -> StepRange<i16>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<i32 >, step: i32) -> StepRange<i32>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<int >, step: int) -> StepRange<int>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<i8 >, step: i8) -> StepRange<i8>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<u128 >, step: u128) -> StepRange<u128>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<u16 >, step: u16) -> StepRange<u16>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<u32 >, step: u32) -> StepRange<u32>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<u64 >, step: u64) -> StepRange<u64>;

/// Return an iterator over an exclusive range, each iteration increasing by `step`.
///
/// If `range` is reversed and `step` < 0, iteration goes backwards.
///
/// Otherwise, if `range` is empty, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8..18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18..8, -3) {
///     print(n);
/// }
/// ```
fn range(range: Range<u8 >, step: u8) -> StepRange<u8>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: float, to: float, step: float) -> StepRange<float>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: i128, to: i128, step: i128) -> StepRange<i128>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: i16, to: i16, step: i16) -> StepRange<i16>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: i32, to: i32, step: i32) -> StepRange<i32>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: int, to: int, step: int) -> StepRange<int>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: i8, to: i8, step: i8) -> StepRange<i8>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: u128, to: u128, step: u128) -> StepRange<u128>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: u16, to: u16, step: u16) -> StepRange<u16>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: u32, to: u32, step: u32) -> StepRange<u32>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: u64, to: u64, step: u64) -> StepRange<u64>;

/// Return an iterator over the exclusive range of `from..to`, each iteration increasing by `step`.
/// The value `to` is never included.
///
/// If `from` > `to` and `step` < 0, iteration goes backwards.
///
/// If `from` > `to` and `step` > 0 or `from` < `to` and `step` < 0, an empty iterator is returned.
///
/// # Example
///
/// ```rhai
/// // prints all values from 8 to 17 in steps of 3
/// for n in range(8, 18, 3) {
///     print(n);
/// }
///
/// // prints all values down from 18 to 9 in steps of -3
/// for n in range(18, 8, -3) {
///     print(n);
/// }
/// ```
fn range(from: u8, to: u8, step: u8) -> StepRange<u8>;

/// Reduce an array by iterating through all elements while applying a function named by `reducer`.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.reduce(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `reducer` must exist taking these parameters:
///
/// * `result`: accumulated result, initially `()`
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn process(r, x) {
///     x + (r ?? 0)
/// }
/// fn process_extra(r, x, i) {
///     x + i + (r ?? 0)
/// }
///
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.reduce("process");
///
/// print(y);       // prints 15
///
/// let y = x.reduce("process_extra");
///
/// print(y);       // prints 25
/// ```
fn reduce(array: array, reducer: string) -> RhaiResult;

/// Reduce an array by iterating through all elements while applying the `reducer` function.
///
/// # Function Parameters
///
/// * `result`: accumulated result, initially `()`
/// * `element`: copy of array element, or bound to `this` if omitted
/// * `index` _(optional)_: current index in the array
///
/// This method is marked _pure_; the `reducer` function should not mutate array elements.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.reduce(|r, v| v + (r ?? 0));
///
/// print(y);       // prints 15
///
/// let y = x.reduce(|r, v, i| v + i + (r ?? 0));
///
/// print(y);       // prints 25
/// ```
fn reduce(array: array, reducer: Fn) -> RhaiResult;

/// Reduce an array by iterating through all elements while applying a function named by `reducer`.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.reduce(Fn("fn_name"), initial)` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `reducer` must exist taking these parameters:
///
/// * `result`: accumulated result, starting with the value of `initial`
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn process(r, x) { x + r }
///
/// fn process_extra(r, x, i) { x + i + r }
///
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.reduce("process", 5);
///
/// print(y);       // prints 20
///
/// let y = x.reduce("process_extra", 5);
///
/// print(y);       // prints 30
/// ```
fn reduce(array: array, reducer: string, initial: ?) -> RhaiResult;

/// Reduce an array by iterating through all elements while applying the `reducer` function.
///
/// # Function Parameters
///
/// * `result`: accumulated result, starting with the value of `initial`
/// * `element`: copy of array element, or bound to `this` if omitted
/// * `index` _(optional)_: current index in the array
///
/// This method is marked _pure_; the `reducer` function should not mutate array elements.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.reduce(|r, v| v + r, 5);
///
/// print(y);       // prints 20
///
/// let y = x.reduce(|r, v, i| v + i + r, 5);
///
/// print(y);       // prints 30
/// ```
fn reduce(array: array, reducer: Fn, initial: ?) -> RhaiResult;

/// Reduce an array by iterating through all elements, in _reverse_ order,
/// while applying a function named by `reducer`.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.reduce_rev(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `reducer` must exist taking these parameters:
///
/// * `result`: accumulated result, initially `()`
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn process(r, x) {
///     x + (r ?? 0)
/// }
/// fn process_extra(r, x, i) {
///     x + i + (r ?? 0)
/// }
///
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.reduce_rev("process");
///
/// print(y);       // prints 15
///
/// let y = x.reduce_rev("process_extra");
///
/// print(y);       // prints 25
/// ```
fn reduce_rev(array: array, reducer: string) -> RhaiResult;

/// Reduce an array by iterating through all elements, in _reverse_ order,
/// while applying the `reducer` function.
///
/// # Function Parameters
///
/// * `result`: accumulated result, initially `()`
/// * `element`: copy of array element, or bound to `this` if omitted
/// * `index` _(optional)_: current index in the array
///
/// This method is marked _pure_; the `reducer` function should not mutate array elements.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.reduce_rev(|r, v| v + (r ?? 0));
///
/// print(y);       // prints 15
///
/// let y = x.reduce_rev(|r, v, i| v + i + (r ?? 0));
///
/// print(y);       // prints 25
/// ```
fn reduce_rev(array: array, reducer: Fn) -> RhaiResult;

/// Reduce an array by iterating through all elements, in _reverse_ order,
/// while applying a function named by `reducer`.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.reduce_rev(Fn("fn_name"), initial)` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `reducer` must exist taking these parameters:
///
/// * `result`: accumulated result, starting with the value of `initial`
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn process(r, x) { x + r }
///
/// fn process_extra(r, x, i) { x + i + r }
///
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.reduce_rev("process", 5);
///
/// print(y);       // prints 20
///
/// let y = x.reduce_rev("process_extra", 5);
///
/// print(y);       // prints 30
/// ```
fn reduce_rev(array: array, reducer: string, initial: ?) -> RhaiResult;

/// Reduce an array by iterating through all elements, in _reverse_ order,
/// while applying the `reducer` function.
///
/// # Function Parameters
///
/// * `result`: accumulated result, starting with the value of `initial`
/// * `element`: copy of array element, or bound to `this` if omitted
/// * `index` _(optional)_: current index in the array
///
/// This method is marked _pure_; the `reducer` function should not mutate array elements.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.reduce_rev(|r, v| v + r, 5);
///
/// print(y);       // prints 20
///
/// let y = x.reduce_rev(|r, v, i| v + i + r, 5);
///
/// print(y);       // prints 30
/// ```
fn reduce_rev(array: array, reducer: Fn, initial: ?) -> RhaiResult;

/// Remove the element at the specified `index` from the array and return it.
///
/// * If `index` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `index` < -length of array, `()` is returned.
/// * If `index` ≥ length of array, `()` is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
///
/// print(x.remove(1));     // prints 2
///
/// print(x);               // prints "[1, 3]"
///
/// print(x.remove(-2));    // prints 1
///
/// print(x);               // prints "[3]"
/// ```
fn remove(array: array, index: int) -> ?;

/// Remove the byte at the specified `index` from the BLOB and return it.
///
/// * If `index` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `index` < -length of BLOB, zero is returned.
/// * If `index` ≥ length of BLOB, zero is returned.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(x.remove(1));     // prints 2
///
/// print(x);               // prints "[01030405]"
///
/// print(x.remove(-2));    // prints 4
///
/// print(x);               // prints "[010305]"
/// ```
fn remove(blob: blob, index: int) -> int;

/// Remove any property of the specified `name` from the object map, returning its value.
///
/// If the property does not exist, `()` is returned.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
///
/// let x = m.remove("b");
///
/// print(x);       // prints 2
///
/// print(m);       // prints "#{a:1, c:3}"
/// ```
fn remove(map: map, property: string) -> ?;

/// Remove all occurrences of a character from the string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// text.remove("o");
///
/// print(text);        // prints "hell, wrld! hell, fbar!"
/// ```
fn remove(string: string, character: char) -> ();

/// Remove all occurrences of a sub-string from the string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// text.remove("hello");
///
/// print(text);        // prints ", world! , foobar!"
/// ```
fn remove(string: string, sub_string: string) -> ();

/// Replace all occurrences of the specified character in the string with another character.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// text.replace("l", '*');
///
/// print(text);        // prints "he**o, wor*d! he**o, foobar!"
/// ```
fn replace(string: string, find_character: char, substitute_character: char) -> ();

/// Replace all occurrences of the specified character in the string with another string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// text.replace('l', "(^)");
///
/// print(text);        // prints "he(^)(^)o, wor(^)d! he(^)(^)o, foobar!"
/// ```
fn replace(string: string, find_character: char, substitute_string: string) -> ();

/// Replace all occurrences of the specified sub-string in the string with the specified character.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// text.replace("hello", '*');
///
/// print(text);        // prints "*, world! *, foobar!"
/// ```
fn replace(string: string, find_string: string, substitute_character: char) -> ();

/// Replace all occurrences of the specified sub-string in the string with another string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// text.replace("hello", "hey");
///
/// print(text);        // prints "hey, world! hey, foobar!"
/// ```
fn replace(string: string, find_string: string, substitute_string: string) -> ();

/// Remove all elements in the array that do not return `true` when applied a function named by
/// `filter` and return them as a new array.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.retain(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `filter` must exist taking these parameters:
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn large(x) { x >= 3 }
///
/// fn screen(x, i) { x + i <= 5 }
///
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.retain("large");
///
/// print(x);       // prints "[3, 4, 5]"
///
/// print(y);       // prints "[1, 2]"
///
/// let z = x.retain("screen");
///
/// print(x);       // prints "[3, 4]"
///
/// print(z);       // prints "[5]"
/// ```
fn retain(array: array, filter: string) -> Array;

/// Remove all elements in the array that do not return `true` when applied the `filter`
/// function and return them as a new array.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.retain(|v| v >= 3);
///
/// print(x);       // prints "[3, 4, 5]"
///
/// print(y);       // prints "[1, 2]"
///
/// let z = x.retain(|v, i| v + i <= 5);
///
/// print(x);       // prints "[3, 4]"
///
/// print(z);       // prints "[5]"
/// ```
fn retain(array: array, filter: Fn) -> Array;

/// Remove all elements in the array not within an exclusive `range` and return them as a new array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.retain(1..4);
///
/// print(x);       // prints "[2, 3, 4]"
///
/// print(y);       // prints "[1, 5]"
///
/// let z = x.retain(1..3);
///
/// print(x);       // prints "[3, 4]"
///
/// print(z);       // prints "[1]"
/// ```
fn retain(array: array, range: range) -> array;

/// Remove all elements in the array not within an inclusive `range` and return them as a new array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.retain(1..=3);
///
/// print(x);       // prints "[2, 3, 4]"
///
/// print(y);       // prints "[1, 5]"
///
/// let z = x.retain(1..=2);
///
/// print(x);       // prints "[3, 4]"
///
/// print(z);       // prints "[1]"
/// ```
fn retain(array: array, range: range=) -> array;

/// Remove all bytes in the BLOB not within an exclusive `range` and return them as a new BLOB.
///
/// # Example
///
/// ```rhai
/// let b1 = blob();
///
/// b1 += 1; b1 += 2; b1 += 3; b1 += 4; b1 += 5;
///
/// let b2 = b1.retain(1..4);
///
/// print(b1);      // prints "[020304]"
///
/// print(b2);      // prints "[0105]"
///
/// let b3 = b1.retain(1..3);
///
/// print(b1);      // prints "[0304]"
///
/// print(b2);      // prints "[01]"
/// ```
fn retain(blob: blob, range: range) -> blob;

/// Remove all bytes in the BLOB not within an inclusive `range` and return them as a new BLOB.
///
/// # Example
///
/// ```rhai
/// let b1 = blob();
///
/// b1 += 1; b1 += 2; b1 += 3; b1 += 4; b1 += 5;
///
/// let b2 = b1.retain(1..=3);
///
/// print(b1);      // prints "[020304]"
///
/// print(b2);      // prints "[0105]"
///
/// let b3 = b1.retain(1..=2);
///
/// print(b1);      // prints "[0304]"
///
/// print(b2);      // prints "[01]"
/// ```
fn retain(blob: blob, range: range=) -> blob;

/// Remove all elements in the object map that do not return `true` when applied the `filter` function and
/// return them as a new object map.
///
/// # Function Parameters
///
/// * `key`: current key
/// * `value` _(optional)_: copy of element (bound to `this` if omitted)
///
/// # Example
///
/// ```rhai
/// let x = #{a:1, b:2, c:3, d:4, e:5};
///
/// let y = x.retain(|k| this < 3);
///
/// print(x);       // prints #{"a":1, "b"2}
///
/// print(y);       // prints #{"c":3, "d":4, "e":5]
///
/// let z = y.retain(|k, v| k == "c" || v >= 5);
///
/// print(y);       // prints #{"c":3, "e":5}
///
/// print(z);       // prints #{"d":4}
/// ```
fn retain(map: map, filter: Fn) -> Map;

/// Remove all elements not within a portion of the array and return them as a new array.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, all elements are removed returned.
/// * If `len` ≤ 0, all elements are removed and returned.
/// * If `start` position + `len` ≥ length of array, entire portion of the array before the `start` position is removed and returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.retain(1, 2);
///
/// print(x);       // prints "[2, 3]"
///
/// print(y);       // prints "[1, 4, 5]"
///
/// let z = x.retain(-1, 1);
///
/// print(x);       // prints "[3]"
///
/// print(z);       // prints "[2]"
/// ```
fn retain(array: array, start: int, len: int) -> array;

/// Remove all bytes not within a portion of the BLOB and return them as a new BLOB.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, all elements are removed returned.
/// * If `len` ≤ 0, all elements are removed and returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB before the `start` position is removed and returned.
///
/// # Example
///
/// ```rhai
/// let b1 = blob();
///
/// b1 += 1; b1 += 2; b1 += 3; b1 += 4; b1 += 5;
///
/// let b2 = b1.retain(1, 2);
///
/// print(b1);      // prints "[0203]"
///
/// print(b2);      // prints "[010405]"
///
/// let b3 = b1.retain(-1, 1);
///
/// print(b1);      // prints "[03]"
///
/// print(b3);      // prints "[02]"
/// ```
fn retain(blob: blob, start: int, len: int) -> blob;

/// Reverse all the elements in the array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// x.reverse();
///
/// print(x);       // prints "[5, 4, 3, 2, 1]"
/// ```
fn reverse(array: array) -> ();

/// Reverse the BLOB.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(b);           // prints "[0102030405]"
///
/// b.reverse();
///
/// print(b);           // prints "[0504030201]"
/// ```
fn reverse(blob: blob) -> ();

/// Return the nearest whole number closest to the floating-point number.
/// Rounds away from zero.
fn round(x: float) -> float;

/// Set the element at the `index` position in the array to a new `value`.
///
/// * If `index` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `index` < -length of array, the array is not modified.
/// * If `index` ≥ length of array, the array is not modified.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
///
/// x.set(0, 42);
///
/// print(x);           // prints "[42, 2, 3]"
///
/// x.set(-3, 0);
///
/// print(x);           // prints "[0, 2, 3]"
///
/// x.set(99, 123);
///
/// print(x);           // prints "[0, 2, 3]"
/// ```
fn set(array: array, index: int, value: ?) -> ();

/// Set the particular `index` position in the BLOB to a new byte `value`.
///
/// * If `index` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `index` < -length of BLOB, the BLOB is not modified.
/// * If `index` ≥ length of BLOB, the BLOB is not modified.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// b.set(0, 0x42);
///
/// print(b);           // prints "[4202030405]"
///
/// b.set(-3, 0);
///
/// print(b);           // prints "[4202000405]"
///
/// b.set(99, 123);
///
/// print(b);           // prints "[4202000405]"
/// ```
fn set(blob: blob, index: int, value: int) -> ();

/// Set the value of the `property` in the object map to a new `value`.
///
/// If `property` does not exist in the object map, it is added.
///
/// # Example
///
/// ```rhai
/// let m = #{a: 1, b: 2, c: 3};
///
/// m.set("b", 42)'
///
/// print(m);           // prints "#{a: 1, b: 42, c: 3}"
///
/// x.set("x", 0);
///
/// print(m);           // prints "#{a: 1, b: 42, c: 3, x: 0}"
/// ```
fn set(map: map, property: string, value: ?) -> ();

/// Set the `index` position in the string to a new `character`.
///
/// * If `index` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `index` < -length of string, the string is not modified.
/// * If `index` ≥ length of string, the string is not modified.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// text.set(3, 'x');
///
/// print(text);     // prints "helxo, world!"
///
/// text.set(-3, 'x');
///
/// print(text);    // prints "hello, worxd!"
///
/// text.set(99, 'x');
///
/// print(text);    // prints "hello, worxd!"
/// ```
fn set(string: string, index: int, character: char) -> ();

/// Set the _tag_ of a `Dynamic` value.
///
/// # Example
///
/// ```rhai
/// let x = "hello, world!";
///
/// x.tag = 42;
///
/// print(x.tag);           // prints 42
/// ```
fn set tag(value: ?, tag: int) -> ();

/// Set the specified `bit` in the number if the new value is `true`.
/// Clear the `bit` if the new value is `false`.
///
/// If `bit` < 0, position counts from the MSB (Most Significant Bit).
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// x.set_bit(5, true);
///
/// print(x);               // prints 123488
///
/// x.set_bit(6, false);
///
/// print(x);               // prints 123424
///
/// x.set_bit(-48, false);
///
/// print(x);               // prints 57888 on 64-bit
/// ```
fn set_bit(value: int, bit: int, new_value: bool) -> ();

/// Replace an exclusive range of bits in the number with a new value.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// x.set_bits(5..10, 42);
///
/// print(x);           // print 123200
/// ```
fn set_bits(value: int, range: range, new_value: int) -> ();

/// Replace an inclusive range of bits in the number with a new value.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// x.set_bits(5..=9, 42);
///
/// print(x);           // print 123200
/// ```
fn set_bits(value: int, range: range=, new_value: int) -> ();

/// Replace a portion of bits in the number with a new value.
///
/// * If `start` < 0, position counts from the MSB (Most Significant Bit).
/// * If `bits` ≤ 0, the number is not modified.
/// * If `start` position + `bits` ≥ total number of bits, the bits after the `start` position are replaced.
///
/// # Example
///
/// ```rhai
/// let x = 123456;
///
/// x.set_bits(5, 8, 42);
///
/// print(x);           // prints 124224
///
/// x.set_bits(-16, 10, 42);
///
/// print(x);           // prints 11821949021971776 on 64-bit
/// ```
fn set_bits(value: int, bit: int, bits: int, new_value: int) -> ();

/// Set the _tag_ of a `Dynamic` value.
///
/// # Example
///
/// ```rhai
/// let x = "hello, world!";
///
/// x.tag = 42;
///
/// print(x.tag);           // prints 42
/// ```
fn set_tag(value: ?, tag: int) -> ();

/// Remove the first element from the array and return it.
///
/// If the array is empty, `()` is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3];
///
/// print(x.shift());   // prints 1
///
/// print(x);           // prints "[2, 3]"
/// ```
fn shift(array: array) -> ?;

/// Remove the first byte from the BLOB and return it.
///
/// If the BLOB is empty, zero is returned.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// print(b.shift());       // prints 1
///
/// print(b);               // prints "[02030405]"
/// ```
fn shift(blob: blob) -> int;

/// Return the sign (as an integer) of the number according to the following:
///
/// * `0` if the number is zero
/// * `1` if the number is positive
/// * `-1` if the number is negative
fn sign(x: int) -> int;

/// Return the sign (as an integer) of the floating-point number according to the following:
///
/// * `0` if the number is zero
/// * `1` if the number is positive
/// * `-1` if the number is negative
fn sign(x: f32) -> int;

/// Return the sign (as an integer) of the floating-point number according to the following:
///
/// * `0` if the number is zero
/// * `1` if the number is positive
/// * `-1` if the number is negative
fn sign(x: float) -> int;

/// Return the sign (as an integer) of the number according to the following:
///
/// * `0` if the number is zero
/// * `1` if the number is positive
/// * `-1` if the number is negative
fn sign(x: i128) -> int;

/// Return the sign (as an integer) of the number according to the following:
///
/// * `0` if the number is zero
/// * `1` if the number is positive
/// * `-1` if the number is negative
fn sign(x: i16) -> int;

/// Return the sign (as an integer) of the number according to the following:
///
/// * `0` if the number is zero
/// * `1` if the number is positive
/// * `-1` if the number is negative
fn sign(x: i32) -> int;

/// Return the sign (as an integer) of the number according to the following:
///
/// * `0` if the number is zero
/// * `1` if the number is positive
/// * `-1` if the number is negative
fn sign(x: i8) -> int;

/// Return the sine of the floating-point number in radians.
fn sin(x: float) -> float;

/// Return the hyperbolic sine of the floating-point number in radians.
fn sinh(x: float) -> float;

/// Block the current thread for a particular number of `seconds`.
///
/// # Example
///
/// ```rhai
/// // Do nothing for 10 seconds!
/// sleep(10.0);
/// ```
fn sleep(seconds: float) -> ();

/// Block the current thread for a particular number of `seconds`.
///
/// # Example
///
/// ```rhai
/// // Do nothing for 10 seconds!
/// sleep(10);
/// ```
fn sleep(seconds: int) -> ();

/// Return `true` if any element in the array that returns `true` when applied a function named
/// by `filter`.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.some(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `filter` must exist taking these parameters:
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// fn large(x) { x > 3 }
///
/// fn huge(x) { x > 10 }
///
/// fn screen(x, i) { i > x }
///
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.some("large"));     // prints true
///
/// print(x.some("huge"));      // prints false
///
/// print(x.some("screen"));    // prints true
/// ```
fn some(array: array, filter: string) -> bool;

/// Return `true` if any element in the array that returns `true` when applied the `filter` function.
///
/// # No Function Parameter
///
/// Array element (mutable) is bound to `this`.
///
/// This method is marked _pure_; the `filter` function should not mutate array elements.
///
/// # Function Parameters
///
/// * `element`: copy of array element
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 5];
///
/// print(x.some(|v| v > 3));       // prints true
///
/// print(x.some(|v| v > 10));      // prints false
///
/// print(x.some(|v, i| i > v));    // prints true
/// ```
fn some(array: array, filter: Fn) -> bool;

/// Sort the array.
///
/// All elements in the array must be of the same data type.
///
/// # Supported Data Types
///
/// * integer numbers
/// * floating-point numbers
/// * decimal numbers
/// * characters
/// * strings
/// * booleans
/// * `()`
///
/// # Example
///
/// ```rhai
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// x.sort();
///
/// print(x);       // prints "[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]"
/// ```
fn sort(array: array) -> ();

/// Sort the array based on applying a function named by `comparer`.
///
/// # Deprecated API
///
/// This method is deprecated and will be removed from the next major version.
/// Use `array.sort(Fn("fn_name"))` instead.
///
/// # Function Parameters
///
/// A function with the same name as the value of `comparer` must exist taking these parameters:
///
/// * `element1`: copy of the current array element to compare
/// * `element2`: copy of the next array element to compare
///
/// ## Return Value
///
/// * Any integer > 0 if `element1 > element2`
/// * Zero if `element1 == element2`
/// * Any integer < 0 if `element1 < element2`
///
/// # Example
///
/// ```rhai
/// fn reverse(a, b) {
///     if a > b {
///         -1
///     } else if a < b {
///         1
///     } else {
///         0
///     }
/// }
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// x.sort("reverse");
///
/// print(x);       // prints "[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]"
/// ```
fn sort(array: array, comparer: string) -> ();

/// Sort the array based on applying the `comparer` function.
///
/// The `comparer` function must implement a [total order](https://en.wikipedia.org/wiki/Total_order).
///
/// # Function Parameters
///
/// * `element1`: copy of the current array element to compare
/// * `element2`: copy of the next array element to compare
///
/// ## Return Value
///
/// An integer number:
///
/// * Any positive integer if `element1 > element2`
/// * 0 if `element1 == element2`
/// * Any negative integer if `element1 < element2`
///
/// or a boolean value:
///
/// * `true` if `element1 <= element2`
/// * `false` if `element1 > element2`
///
/// Any other return value type will yield unpredictable order.
///
/// # Errors
///
/// If the `comparer` function does not implement a [total order](https://en.wikipedia.org/wiki/Total_order),
/// an error is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// // Do comparisons in reverse
/// x.sort_by(|a, b| if a > b { -1 } else if a < b { 1 } else { 0 });
///
/// print(x);       // prints "[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]"
/// ```
fn sort(array: array, comparer: Fn) -> ();

/// Sort the array based on applying the `comparer` function.
///
/// The `comparer` function must implement a [total order](https://en.wikipedia.org/wiki/Total_order).
///
/// # Function Parameters
///
/// * `element1`: copy of the current array element to compare
/// * `element2`: copy of the next array element to compare
///
/// ## Return Value
///
/// An integer number:
///
/// * Any positive integer if `element1 > element2`
/// * 0 if `element1 == element2`
/// * Any negative integer if `element1 < element2`
///
/// or a boolean value:
///
/// * `true` if `element1 <= element2`
/// * `false` if `element1 > element2`
///
/// Any other return value type will yield unpredictable order.
///
/// # Errors
///
/// If the `comparer` function does not implement a [total order](https://en.wikipedia.org/wiki/Total_order),
/// an error is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// // Do comparisons in reverse
/// x.sort_by(|a, b| if a > b { -1 } else if a < b { 1 } else { 0 });
///
/// print(x);       // prints "[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]"
/// ```
fn sort_by(array: array, comparer: Fn) -> ();

/// Sort the array in descending order.
///
/// All elements in the array must be of the same data type.
///
/// # Supported Data Types
///
/// * integer numbers
/// * floating-point numbers
/// * decimal numbers
/// * characters
/// * strings
/// * booleans
/// * `()`
///
/// # Example
///
/// ```rhai
/// let x = [1, 3, 5, 7, 9, 2, 4, 6, 8, 10];
///
/// x.sort_desc();
///
/// print(x);       // prints "[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]"
/// ```
fn sort_desc(array: array) -> ();

/// Replace an exclusive range of the array with another array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
/// let y = [7, 8, 9, 10];
///
/// x.splice(1..3, y);
///
/// print(x);       // prints "[1, 7, 8, 9, 10, 4, 5]"
/// ```
fn splice(array: array, range: range, replace: array) -> ();

/// Replace an inclusive range of the array with another array.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
/// let y = [7, 8, 9, 10];
///
/// x.splice(1..=3, y);
///
/// print(x);       // prints "[1, 7, 8, 9, 10, 5]"
/// ```
fn splice(array: array, range: range=, replace: array) -> ();

/// Replace an exclusive `range` of the BLOB with another BLOB.
///
/// # Example
///
/// ```rhai
/// let b1 = blob(10, 0x42);
/// let b2 = blob(5, 0x18);
///
/// b1.splice(1..4, b2);
///
/// print(b1);      // prints "[4218181818184242 42424242]"
/// ```
fn splice(blob: blob, range: range, replace: blob) -> ();

/// Replace an inclusive `range` of the BLOB with another BLOB.
///
/// # Example
///
/// ```rhai
/// let b1 = blob(10, 0x42);
/// let b2 = blob(5, 0x18);
///
/// b1.splice(1..=4, b2);
///
/// print(b1);      // prints "[4218181818184242 424242]"
/// ```
fn splice(blob: blob, range: range=, replace: blob) -> ();

/// Replace a portion of the array with another array.
///
/// * If `start` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `start` < -length of array, position counts from the beginning of the array.
/// * If `start` ≥ length of array, the other array is appended to the end of the array.
/// * If `len` ≤ 0, the other array is inserted into the array at the `start` position without replacing any element.
/// * If `start` position + `len` ≥ length of array, entire portion of the array after the `start` position is replaced.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
/// let y = [7, 8, 9, 10];
///
/// x.splice(1, 2, y);
///
/// print(x);       // prints "[1, 7, 8, 9, 10, 4, 5]"
///
/// x.splice(-5, 4, y);
///
/// print(x);       // prints "[1, 7, 7, 8, 9, 10, 5]"
/// ```
fn splice(array: array, start: int, len: int, replace: array) -> ();

/// Replace a portion of the BLOB with another BLOB.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, the other BLOB is appended to the end of the BLOB.
/// * If `len` ≤ 0, the other BLOB is inserted into the BLOB at the `start` position without replacing anything.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is replaced.
///
/// # Example
///
/// ```rhai
/// let b1 = blob(10, 0x42);
/// let b2 = blob(5, 0x18);
///
/// b1.splice(1, 3, b2);
///
/// print(b1);      // prints "[4218181818184242 42424242]"
///
/// b1.splice(-5, 4, b2);
///
/// print(b1);      // prints "[4218181818184218 1818181842]"
/// ```
fn splice(blob: blob, start: int, len: int, replace: blob) -> ();

/// Split the string into segments based on whitespaces, returning an array of the segments.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split());        // prints ["hello,", "world!", "hello,", "foo!"]
/// ```
fn split(string: string) -> array;

/// Cut off the array at `index` and return it as a new array.
///
/// * If `index` < 0, position counts from the end of the array (`-1` is the last element).
/// * If `index` is zero, the entire array is cut and returned.
/// * If `index` < -length of array, the entire array is cut and returned.
/// * If `index` ≥ length of array, nothing is cut from the array and an empty array is returned.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// let y = x.split(2);
///
/// print(y);           // prints "[3, 4, 5]"
///
/// print(x);           // prints "[1, 2]"
/// ```
fn split(array: array, index: int) -> array;

/// Cut off the BLOB at `index` and return it as a new BLOB.
///
/// * If `index` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `index` is zero, the entire BLOB is cut and returned.
/// * If `index` < -length of BLOB, the entire BLOB is cut and returned.
/// * If `index` ≥ length of BLOB, nothing is cut from the BLOB and an empty BLOB is returned.
///
/// # Example
///
/// ```rhai
/// let b1 = blob();
///
/// b1 += 1; b1 += 2; b1 += 3; b1 += 4; b1 += 5;
///
/// let b2 = b1.split(2);
///
/// print(b2);          // prints "[030405]"
///
/// print(b1);          // prints "[0102]"
/// ```
fn split(blob: blob, index: int) -> blob;

/// Split the string into segments based on a `delimiter` string, returning an array of the segments.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split("ll"));    // prints ["he", "o, world! he", "o, foo!"]
/// ```
fn split(string: string, delimiter: string) -> array;

/// Split the string into segments based on a `delimiter` character, returning an array of the segments.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split('l'));     // prints ["he", "", "o, wor", "d! he", "", "o, foo!"]
/// ```
fn split(string: string, delimiter: char) -> array;

/// Split the string into two at the specified `index` position and return it both strings
/// as an array.
///
/// The character at the `index` position (if any) is returned in the _second_ string.
///
/// * If `index` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `index` < -length of string, it is equivalent to cutting at position 0.
/// * If `index` ≥ length of string, it is equivalent to cutting at the end of the string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.split(6));       // prints ["hello,", " world!"]
///
/// print(text.split(13));      // prints ["hello, world!", ""]
///
/// print(text.split(-6));      // prints ["hello, ", "world!"]
///
/// print(text.split(-99));     // prints ["", "hello, world!"]
/// ```
fn split(string: string, index: int) -> array;

/// Split the string into at most the specified number of `segments` based on a `delimiter` string,
/// returning an array of the segments.
///
/// If `segments` < 1, only one segment is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split("ll", 2));     // prints ["he", "o, world! hello, foo!"]
/// ```
fn split(string: string, delimiter: string, segments: int) -> array;

/// Split the string into at most the specified number of `segments` based on a `delimiter` character,
/// returning an array of the segments.
///
/// If `segments` < 1, only one segment is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split('l', 3));      // prints ["he", "", "o, world! hello, foo!"]
/// ```
fn split(string: string, delimiter: char, segments: int) -> array;

/// Split the string into segments based on a `delimiter` string, returning an array of the
/// segments in _reverse_ order.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split_rev("ll"));    // prints ["o, foo!", "o, world! he", "he"]
/// ```
fn split_rev(string: string, delimiter: string) -> array;

/// Split the string into segments based on a `delimiter` character, returning an array of
/// the segments in _reverse_ order.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split_rev('l'));     // prints ["o, foo!", "", "d! he", "o, wor", "", "he"]
/// ```
fn split_rev(string: string, delimiter: char) -> array;

/// Split the string into at most a specified number of `segments` based on a `delimiter` string,
/// returning an array of the segments in _reverse_ order.
///
/// If `segments` < 1, only one segment is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split_rev("ll", 2));     // prints ["o, foo!", "hello, world! he"]
/// ```
fn split_rev(string: string, delimiter: string, segments: int) -> array;

/// Split the string into at most the specified number of `segments` based on a `delimiter` character,
/// returning an array of the segments.
///
/// If `segments` < 1, only one segment is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foo!";
///
/// print(text.split('l', 3));      // prints ["o, foo!", "", "hello, world! he"
/// ```
fn split_rev(string: string, delimiter: char, segments: int) -> array;

/// Return the square root of the floating-point number.
fn sqrt(x: float) -> float;

/// Return the start of the exclusive range.
fn start(range: range) -> int;

/// Return the start of the inclusive range.
fn start(range: range=) -> int;

/// Return `true` if the string starts with a specified character.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.starts_with('h'));       // prints true
///
/// print(text.starts_with('w'));       // prints false
/// ```
fn starts_with(string: string, character: char) -> bool;

/// Return `true` if the string starts with a specified string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.starts_with("hello"));   // prints true
///
/// print(text.starts_with("world"));   // prints false
/// ```
fn starts_with(string: string, match_string: string) -> bool;

/// Copy an exclusive range of characters from the string and return it as a new string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.sub_string(3..7));   // prints "lo, "
/// ```
fn sub_string(string: string, range: range) -> string;

/// Copy an inclusive range of characters from the string and return it as a new string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.sub_string(3..=7));  // prints "lo, w"
/// ```
fn sub_string(string: string, range: range=) -> string;

/// Copy a portion of the string beginning at the `start` position till the end and return it as
/// a new string.
///
/// * If `start` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `start` < -length of string, the entire string is copied and returned.
/// * If `start` ≥ length of string, an empty string is returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.sub_string(5));      // prints ", world!"
///
/// print(text.sub_string(-5));      // prints "orld!"
/// ```
fn sub_string(string: string, start: int) -> string;

/// Copy a portion of the string and return it as a new string.
///
/// * If `start` < 0, position counts from the end of the string (`-1` is the last character).
/// * If `start` < -length of string, position counts from the beginning of the string.
/// * If `start` ≥ length of string, an empty string is returned.
/// * If `len` ≤ 0, an empty string is returned.
/// * If `start` position + `len` ≥ length of string, entire portion of the string after the `start` position is copied and returned.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!";
///
/// print(text.sub_string(3, 4));   // prints "lo, "
///
/// print(text.sub_string(-8, 3));  // prints ", w"
/// ```
fn sub_string(string: string, start: int, len: int) -> string;

/// Return the _tag_ of a `Dynamic` value.
///
/// # Example
///
/// ```rhai
/// let x = "hello, world!";
///
/// x.tag = 42;
///
/// print(x.tag);           // prints 42
/// ```
fn tag(value: ?) -> int;

/// Take ownership of the data in a `Dynamic` value and return it.
/// The data is _NOT_ cloned.
///
/// The original value is replaced with `()`.
///
/// # Example
///
/// ```rhai
/// let x = 42;
///
/// print(take(x));         // prints 42
///
/// print(x);               // prints ()
/// ```
fn take(value: ?) -> RhaiResult;

/// Return the tangent of the floating-point number in radians.
fn tan(x: float) -> float;

/// Return the hyperbolic tangent of the floating-point number in radians.
fn tanh(x: float) -> float;

/// Create a timestamp containing the current system time.
///
/// # Example
///
/// ```rhai
/// let now = timestamp();
///
/// sleep(10.0);            // sleep for 10 seconds
///
/// print(now.elapsed);     // prints 10.???
/// ```
fn timestamp() -> timestamp;

/// Convert the BLOB into an array of integers.
///
/// # Example
///
/// ```rhai
/// let b = blob(5, 0x42);
///
/// let x = b.to_array();
///
/// print(x);       // prints "[66, 66, 66, 66, 66]"
/// ```
fn to_array(blob: blob) -> array;

/// Convert the `value` into a string in binary format.
fn to_binary(value: i128) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: i16) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: i32) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: int) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: i8) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: u128) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: u16) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: u32) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: u64) -> string;

/// Convert the `value` into a string in binary format.
fn to_binary(value: u8) -> string;

/// Convert the string into an UTF-8 encoded byte-stream as a BLOB.
///
/// # Example
///
/// ```rhai
/// let text = "朝には紅顔ありて夕べには白骨となる";
///
/// let bytes = text.to_blob();
///
/// print(bytes.len());     // prints 51
/// ```
fn to_blob(string: string) -> blob;

/// Return an array containing all the characters of the string.
///
/// # Example
///
/// ```rhai
/// let text = "hello";
///
/// print(text.to_chars());     // prints "['h', 'e', 'l', 'l', 'o']"
/// ```
fn to_chars(string: string) -> array;

/// Convert the array into a string.
fn to_debug(array: array) -> string;

/// Convert the string into debug format.
fn to_debug(character: char) -> string;

/// Convert the function pointer into a string in debug format.
fn to_debug(f: Fn) -> string;

/// Convert the value of the `item` into a string in debug format.
fn to_debug(item: ?) -> string;

/// Convert the object map into a string.
fn to_debug(map: map) -> string;

/// Convert the value of `number` into a string.
fn to_debug(number: f32) -> string;

/// Convert the value of `number` into a string.
fn to_debug(number: float) -> string;

/// Convert the string into debug format.
fn to_debug(string: string) -> string;

/// Convert the unit into a string in debug format.
fn to_debug(unit: ()) -> string;

/// Convert the boolean value into a string in debug format.
fn to_debug(value: bool) -> string;

/// Convert radians to degrees.
fn to_degrees(x: float) -> float;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

fn to_float(_: ?) -> ;

/// Convert the 32-bit floating-point number to 64-bit.
fn to_float(x: f32) -> float;

/// Convert the `value` into a string in hex format.
fn to_hex(value: i128) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: i16) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: i32) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: int) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: i8) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: u128) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: u16) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: u32) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: u64) -> string;

/// Convert the `value` into a string in hex format.
fn to_hex(value: u8) -> string;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

fn to_int(_: ?) -> ;

/// Convert the floating-point number into an integer.
fn to_int(x: f32) -> int;

/// Convert the floating-point number into an integer.
fn to_int(x: float) -> int;

/// Return the JSON representation of the object map.
///
/// # Data types
///
/// Only the following data types should be kept inside the object map:
/// `INT`, `FLOAT`, `ImmutableString`, `char`, `bool`, `()`, `Array`, `Map`.
///
/// # Errors
///
/// Data types not supported by JSON serialize into formats that may
/// invalidate the result.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
///
/// print(m.to_json());     // prints {"a":1, "b":2, "c":3}
/// ```
fn to_json(map: map) -> String;

/// Convert the character to lower-case and return it as a new character.
///
/// # Example
///
/// ```rhai
/// let ch = 'A';
///
/// print(ch.to_lower());       // prints 'a'
///
/// print(ch);                  // prints 'A'
/// ```
fn to_lower(character: char) -> char;

/// Convert the string to all lower-case and return it as a new string.
///
/// # Example
///
/// ```rhai
/// let text = "HELLO, WORLD!"
///
/// print(text.to_lower());     // prints "hello, world!"
///
/// print(text);                // prints "HELLO, WORLD!"
/// ```
fn to_lower(string: string) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: i128) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: i16) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: i32) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: int) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: i8) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: u128) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: u16) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: u32) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: u64) -> string;

/// Convert the `value` into a string in octal format.
fn to_octal(value: u8) -> string;

/// Convert degrees to radians.
fn to_radians(x: float) -> float;

/// Convert the array into a string.
fn to_string(array: array) -> string;

/// Return the character into a string.
fn to_string(character: char) -> string;

/// Convert the value of the `item` into a string.
fn to_string(item: ?) -> string;

/// Convert the object map into a string.
fn to_string(map: map) -> string;

/// Convert the value of `number` into a string.
fn to_string(number: f32) -> string;

/// Convert the value of `number` into a string.
fn to_string(number: float) -> string;

/// Return the `string`.
fn to_string(string: string) -> string;

/// Return the empty string.
fn to_string(unit: ()) -> string;

/// Return the boolean value into a string.
fn to_string(value: bool) -> string;

/// Convert the character to upper-case and return it as a new character.
///
/// # Example
///
/// ```rhai
/// let ch = 'a';
///
/// print(ch.to_upper());       // prints 'A'
///
/// print(ch);                  // prints 'a'
/// ```
fn to_upper(character: char) -> char;

/// Convert the string to all upper-case and return it as a new string.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world!"
///
/// print(text.to_upper());     // prints "HELLO, WORLD!"
///
/// print(text);                // prints "hello, world!"
/// ```
fn to_upper(string: string) -> string;

/// Remove whitespace characters from both ends of the string.
///
/// # Example
///
/// ```rhai
/// let text = "   hello     ";
///
/// text.trim();
///
/// print(text);    // prints "hello"
/// ```
fn trim(string: string) -> ();

/// Cut off the array at the specified length.
///
/// * If `len` ≤ 0, the array is cleared.
/// * If `len` ≥ length of array, the array is not truncated.
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
///
/// x.truncate(3);
///
/// print(x);       // prints "[1, 2, 3]"
///
/// x.truncate(10);
///
/// print(x);       // prints "[1, 2, 3]"
/// ```
fn truncate(array: array, len: int) -> ();

/// Cut off the BLOB at the specified length.
///
/// * If `len` ≤ 0, the BLOB is cleared.
/// * If `len` ≥ length of BLOB, the BLOB is not truncated.
///
/// # Example
///
/// ```rhai
/// let b = blob();
///
/// b += 1; b += 2; b += 3; b += 4; b += 5;
///
/// b.truncate(3);
///
/// print(b);           // prints "[010203]"
///
/// b.truncate(10);
///
/// print(b);           // prints "[010203]"
/// ```
fn truncate(blob: blob, len: int) -> ();

/// Cut off the string at the specified number of characters.
///
/// * If `len` ≤ 0, the string is cleared.
/// * If `len` ≥ length of string, the string is not truncated.
///
/// # Example
///
/// ```rhai
/// let text = "hello, world! hello, foobar!";
///
/// text.truncate(13);
///
/// print(text);    // prints "hello, world!"
///
/// text.truncate(10);
///
/// print(text);    // prints "hello, world!"
/// ```
fn truncate(string: string, len: int) -> ();

/// Return an array with all the property values in the object map.
///
/// # Example
///
/// ```rhai
/// let m = #{a:1, b:2, c:3};
///
/// print(m.values());      // prints "[1, 2, 3]""
/// ```
fn values(map: map) -> array;

/// Write an ASCII string to the bytes within an exclusive `range` in the BLOB.
///
/// Each ASCII character encodes to one single byte in the BLOB.
/// Non-ASCII characters are ignored.
///
/// * If number of bytes in `range` < length of `string`, extra bytes in `string` are not written.
/// * If number of bytes in `range` > length of `string`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_ascii(1..5, "hello, world!");
///
/// print(b);       // prints "[0068656c6c000000]"
/// ```
fn write_ascii(blob: blob, range: range, string: string) -> ();

/// Write an ASCII string to the bytes within an inclusive `range` in the BLOB.
///
/// Each ASCII character encodes to one single byte in the BLOB.
/// Non-ASCII characters are ignored.
///
/// * If number of bytes in `range` < length of `string`, extra bytes in `string` are not written.
/// * If number of bytes in `range` > length of `string`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_ascii(1..=5, "hello, world!");
///
/// print(b);       // prints "[0068656c6c6f0000]"
/// ```
fn write_ascii(blob: blob, range: range=, string: string) -> ();

/// Write an ASCII string to the bytes within an exclusive `range` in the BLOB.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, the BLOB is not modified.
/// * If `len` ≤ 0, the BLOB is not modified.
/// * If `start` position + `len` ≥ length of BLOB, only the portion of the BLOB after the `start` position is modified.
///
/// * If number of bytes in `range` < length of `string`, extra bytes in `string` are not written.
/// * If number of bytes in `range` > length of `string`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_ascii(1, 5, "hello, world!");
///
/// print(b);       // prints "[0068656c6c6f0000]"
/// ```
fn write_ascii(blob: blob, start: int, len: int, string: string) -> ();

/// Write a `FLOAT` value to the bytes within an exclusive `range` in the BLOB
/// in big-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, extra bytes in `FLOAT` are not written.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes in `range` are not modified.
fn write_be(blob: blob, range: range, value: float) -> ();

/// Write an `INT` value to the bytes within an exclusive `range` in the BLOB
/// in big-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `INT`, extra bytes in `INT` are not written.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8, 0x42);
///
/// b.write_be_int(1..3, 0x99);
///
/// print(b);       // prints "[4200004242424242]"
/// ```
fn write_be(blob: blob, range: range, value: int) -> ();

/// Write a `FLOAT` value to the bytes within an inclusive `range` in the BLOB
/// in big-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, extra bytes in `FLOAT` are not written.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes in `range` are not modified.
fn write_be(blob: blob, range: range=, value: float) -> ();

/// Write an `INT` value to the bytes within an inclusive `range` in the BLOB
/// in big-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `INT`, extra bytes in `INT` are not written.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8, 0x42);
///
/// b.write_be_int(1..=3, 0x99);
///
/// print(b);       // prints "[4200000042424242]"
/// ```
fn write_be(blob: blob, range: range=, value: int) -> ();

/// Write a `FLOAT` value to the bytes beginning at the `start` position in the BLOB
/// in big-endian byte order.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, zero is returned.
/// * If `len` ≤ 0, zero is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is parsed.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, extra bytes in `FLOAT` are not written.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes in `range` are not modified.
fn write_be(blob: blob, start: int, len: int, value: float) -> ();

/// Write an `INT` value to the bytes beginning at the `start` position in the BLOB
/// in big-endian byte order.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, zero is returned.
/// * If `len` ≤ 0, zero is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is parsed.
///
/// * If number of bytes in `range` < number of bytes for `INT`, extra bytes in `INT` are not written.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8, 0x42);
///
/// b.write_be_int(1, 3, 0x99);
///
/// print(b);       // prints "[4200000042424242]"
/// ```
fn write_be(blob: blob, start: int, len: int, value: int) -> ();

/// Write a `FLOAT` value to the bytes within an exclusive `range` in the BLOB
/// in little-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, extra bytes in `FLOAT` are not written.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes in `range` are not modified.
fn write_le(blob: blob, range: range, value: float) -> ();

/// Write an `INT` value to the bytes within an exclusive `range` in the BLOB
/// in little-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `INT`, extra bytes in `INT` are not written.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_le_int(1..3, 0x12345678);
///
/// print(b);       // prints "[0078560000000000]"
/// ```
fn write_le(blob: blob, range: range, value: int) -> ();

/// Write a `FLOAT` value to the bytes within an inclusive `range` in the BLOB
/// in little-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, extra bytes in `FLOAT` are not written.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes in `range` are not modified.
fn write_le(blob: blob, range: range=, value: float) -> ();

/// Write an `INT` value to the bytes within an inclusive `range` in the BLOB
/// in little-endian byte order.
///
/// * If number of bytes in `range` < number of bytes for `INT`, extra bytes in `INT` are not written.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_le_int(1..=3, 0x12345678);
///
/// print(b);       // prints "[0078563400000000]"
/// ```
fn write_le(blob: blob, range: range=, value: int) -> ();

/// Write a `FLOAT` value to the bytes beginning at the `start` position in the BLOB
/// in little-endian byte order.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, zero is returned.
/// * If `len` ≤ 0, zero is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is parsed.
///
/// * If number of bytes in `range` < number of bytes for `FLOAT`, extra bytes in `FLOAT` are not written.
/// * If number of bytes in `range` > number of bytes for `FLOAT`, extra bytes in `range` are not modified.
fn write_le(blob: blob, start: int, len: int, value: float) -> ();

/// Write an `INT` value to the bytes beginning at the `start` position in the BLOB
/// in little-endian byte order.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, zero is returned.
/// * If `len` ≤ 0, zero is returned.
/// * If `start` position + `len` ≥ length of BLOB, entire portion of the BLOB after the `start` position is parsed.
///
/// * If number of bytes in `range` < number of bytes for `INT`, extra bytes in `INT` are not written.
/// * If number of bytes in `range` > number of bytes for `INT`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_le_int(1, 3, 0x12345678);
///
/// print(b);       // prints "[0078563400000000]"
/// ```
fn write_le(blob: blob, start: int, len: int, value: int) -> ();

/// Write a string to the bytes within an exclusive `range` in the BLOB in UTF-8 encoding.
///
/// * If number of bytes in `range` < length of `string`, extra bytes in `string` are not written.
/// * If number of bytes in `range` > length of `string`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_utf8(1..5, "朝には紅顔ありて夕べには白骨となる");
///
/// print(b);       // prints "[00e69c9de3000000]"
/// ```
fn write_utf8(blob: blob, range: range, string: string) -> ();

/// Write a string to the bytes within an inclusive `range` in the BLOB in UTF-8 encoding.
///
/// * If number of bytes in `range` < length of `string`, extra bytes in `string` are not written.
/// * If number of bytes in `range` > length of `string`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_utf8(1..=5, "朝には紅顔ありて夕べには白骨となる");
///
/// print(b);       // prints "[00e69c9de3810000]"
/// ```
fn write_utf8(blob: blob, range: range=, string: string) -> ();

/// Write a string to the bytes within an inclusive `range` in the BLOB in UTF-8 encoding.
///
/// * If `start` < 0, position counts from the end of the BLOB (`-1` is the last byte).
/// * If `start` < -length of BLOB, position counts from the beginning of the BLOB.
/// * If `start` ≥ length of BLOB, the BLOB is not modified.
/// * If `len` ≤ 0, the BLOB is not modified.
/// * If `start` position + `len` ≥ length of BLOB, only the portion of the BLOB after the `start` position is modified.
///
/// * If number of bytes in `range` < length of `string`, extra bytes in `string` are not written.
/// * If number of bytes in `range` > length of `string`, extra bytes in `range` are not modified.
///
/// ```rhai
/// let b = blob(8);
///
/// b.write_utf8(1, 5, "朝には紅顔ありて夕べには白骨となる");
///
/// print(b);       // prints "[00e69c9de3810000]"
/// ```
fn write_utf8(blob: blob, start: int, len: int, string: string) -> ();

/// Iterate through all elements in two arrays, applying a `mapper` function to them,
/// and return a new array containing the results.
///
/// # Function Parameters
///
/// * `array1`: First array
/// * `array2`: Second array
/// * `index` _(optional)_: current index in the array
///
/// # Example
///
/// ```rhai
/// let x = [1, 2, 3, 4, 5];
/// let y = [9, 8, 7, 6];
///
/// let z = x.zip(y, |a, b| a + b);
///
/// print(z);       // prints [10, 10, 10, 10]
///
/// let z = x.zip(y, |a, b, i| a + b + i);
///
/// print(z);       // prints [10, 11, 12, 13]
/// ```
fn zip(array1: array, array2: array, map: Fn) -> Array;

op |(i128, i128) -> i128;

op |(i16, i16) -> i16;

op |(i32, i32) -> i32;

op |(i8, i8) -> i8;

op |(u128, u128) -> u128;

op |(u16, u16) -> u16;

op |(u32, u32) -> u32;

op |(u64, u64) -> u64;

op |(u8, u8) -> u8;

module State {
const Established: CallState;

const Idle: CallState;

const Ringing: CallState;
}


