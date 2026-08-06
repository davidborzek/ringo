// Type definitions for the ringo-flow scenario DSL (JS/TS frontend).
// GENERATED — do not edit. Interface methods + globals are derived from the Rust
// binding signatures (#[ts_export] / #[ts_global]); data-shape types from
// #[derive(TsInterface|TsEnum)] / declare!. Regenerate with
// `ringo-flow definitions --lang js`.

/** A regex path matcher built with `regex(...)`, for the mock server's path args. */
interface PathMatch { readonly __pathMatch?: never; }

/** A static response, or a closure invoked per request (runs on the scenario
 *  thread, pumped from `until`, so it may close over scenario state). */
type MockResponder = MockResponseSpec | ((req: MockRequestInfo) => MockResponseSpec);

declare const enum State { Idle = "idle", Ringing = "ringing", Established = "established" }

declare namespace scenario {
  function each<T>(table: T[]): ScenarioEachFactory<T>;
}

interface AgentConfig {
  /** SIP user (registration / auth). Required. */
  username: string;
  /** SIP domain / registrar. Required. */
  domain: string;
  /** Auth password. */
  password?: string;
  /** `udp` (default), `tcp` or `tls`. */
  transport?: string;
  /** auth user, if it differs from `username`. */
  auth_user?: string;
  /** caller display name. */
  display_name?: string;
  /** outbound proxy URI. */
  outbound?: string;
  /** STUN server, e.g. `stun:host:port`. */
  stun_server?: string;
  /** media encryption, e.g. `srtp`, `zrtp`, `dtls_srtp`. */
  media_enc?: string;
  /** re-registration interval (seconds); `0` disables. */
  regint?: number;
  /** subscribe to message-waiting indication. */
  mwi?: boolean;
  /** `"info"` for reliable headless DTMF (SIP INFO). */
  dtmf_mode?: string;
  /** extra SIP headers on the INVITE, e.g. `{ "X-Foo": "bar" }`; an array value */
  /** (`{ "X-Foo": ["a", "b"] }`) sends the header repeated, once per element. */
  headers?: Record<string, string | string[]>;
  /** deflect inbound calls with a 302 to this URI/number. */
  deflect_to?: string;
  /** free-form data carried on the agent, read back as `agent.metadata` */
  /** (e.g. `{ role: "caller" }`); not used for SIP. */
  metadata?: Record<string, unknown>;
}

interface AgentInfo {
  /** The agent's name (as passed to `new Agent(name, …)`). */
  name: string;
  /** The agent's address-of-record (`sip:user@domain`). */
  aor: string;
  /** Whether the agent is currently registered. */
  registered: boolean;
  /** Current call phase (compare against `State.*`). */
  state: State;
  /** SIP reason phrase of the last response, if any. */
  reason?: string;
  /** SIP status code of the last response, if any. */
  statusCode?: number;
  /** The current call's remote party, if there is a call. */
  peer?: Peer;
  /** Number of active calls on this agent. */
  calls: number;
}

interface AudioSpec { readonly __audioSpec?: never; }

interface CallQuality {
  /** Mean Opinion Score (1.0–5.0); higher is better. */
  readonly mos: number;
  /** Round-trip time in milliseconds. */
  readonly rtt: number;
  /** Jitter in milliseconds. */
  readonly jitter: number;
  /** Receive-side packet loss, in percent (0.0–100.0). */
  readonly packetLoss: number;
}

interface HttpOptions {
  /** Request headers to send. */
  headers?: Record<string, string>;
  /** request body; an object is JSON-encoded. */
  body?: string | object;
}

interface MockRequestInfo {
  /** HTTP method (`GET`, `POST`, …). */
  method: string;
  /** Request path (without query string). */
  path: string;
  /** Parsed query-string parameters. */
  query: Record<string, string>;
  /** Request headers. */
  headers: Record<string, string>;
  /** Raw request body. */
  body: string;
}

interface MockResponseSpec {
  /** HTTP status code to return (default `200`). */
  status?: number;
  /** Response body (a string; use `jsonResponse`/`textResponse` for shorthands). */
  body?: string;
  /** `Content-Type` header to set. */
  contentType?: string;
  /** Extra response headers. */
  headers?: Record<string, string>;
}

interface Peer {
  /** Full SIP URI of the remote party (e.g. `sip:bob@example.com`). */
  readonly uri: string;
  /** The remote party's number / user part. */
  readonly number: string;
  /** The remote party's display name, if the call signalled one. */
  readonly name?: string;
}

interface ScenarioEachFactory<T> {
  (name: string, body: ScenarioEachBody<T>): void;
  (name: string, opts: ScenarioOptions, body: ScenarioEachBody<T>): void;
}

interface ScenarioOptions {
  /** Tags for filtering with `--tag` / `--exclude-tag`. */
  tags?: string[];
  /** `true` to skip, or a string reason (reported, not run). */
  skip?: boolean | string;
  /** If any scenario sets `only: true`, only those run. */
  only?: boolean;
}

type ScenarioBody = (ctx: any) => void | Promise<void>;

type ScenarioEachBody<T> = (ctx: any, param: T) => void | Promise<void>;

declare class Agent {
  constructor(name: string, config: AgentConfig);
  readonly registered: boolean;
  readonly state: State;
  /** RTP media quality of the active/last call (`{ mos, rtt, jitter, packetLoss }`),
   *  or `undefined` until metrics are available (no RTCP report yet). */
  readonly quality?: CallQuality;
  readonly receivedDtmf: string;
  /** Free-form metadata attached at `new Agent(...)` via the `metadata` config field
   *  (e.g. `caller.metadata.role`). Empty object if none was given. */
  readonly metadata: Record<string, unknown>;
  register(): void;
  accept(): void;
  hangup(): void;
  /** `dtmf(digits)` sends back-to-back; `dtmf(digits, gap)` inserts a pause
   *  (e.g. `"200ms"`) between digits. */
  dtmf(digits: string, gap?: string): void;
  /** Dial a target: another `Agent` (at its AOR) or a SIP URI / number string. */
  dial(target: Agent | string): void;
  hold(): void;
  resume(): void;
  mute(): void;
  /** Blind-transfer the current call to a target: another `Agent` or a URI string. */
  transfer(target: Agent | string): void;
  /** Start an attended transfer to a target: another `Agent` or a URI string. */
  attendedTransfer(target: Agent | string): void;
  completeTransfer(): void;
  abortTransfer(): void;
  /** Deflect inbound calls (302) to a target: another `Agent` or a URI / number string. */
  deflect(target: Agent | string): void;
  stopDeflect(): void;
  /** Answer inbound INVITEs with a custom SIP response instead of accepting.
   *  `respondIncoming(486, "Busy Here")`, or with extra header lines:
   *  `respondIncoming(302, "Moved Temporarily", { Contact: "<sip:bob@example.com>" })`. */
  respondIncoming(code: number, reason: string, headers?: Record<string, string>): void;
  /** A snapshot of the agent's observable state as an object. (For a JSON string,
   *  just `JSON.stringify(agent.info())`.) */
  info(): AgentInfo;
  readonly reason?: string;
  readonly statusCode?: number;
  /** First value of a received INVITE header (`a.header("X-Trace-Id")`). */
  header(name: string): string | undefined;
  /** All received INVITE headers as `{ name: [value, …] }` (repeated headers keep every value). */
  readonly headers: Record<string, string[]>;
  /** The current call's remote party: `{ uri, number, name }`, or `undefined`. */
  readonly peer?: Peer;
  /** Set this agent's audio source on the active call (`a.sendAudio(tone(440))`). */
  sendAudio(spec: AudioSpec): void;
  /** Assert the agent receives a `freq` Hz tone within `within` (e.g. `"5s"`).
   *  Returns a Promise: the blocking detection window runs on the runtime's
   *  blocking pool, so `await Promise.all([a.verifyAudio(...), b.verifyAudio(...)])`
   *  listens on several agents concurrently instead of serially. */
  verifyAudio(freq: number, within: string): Promise<void>;
}

interface Assertion<T> {
  /** Negate the next matcher (Jest-style): `expect(x).not.toBe(2)`. Applies only to
   *  the matcher immediately after — the handle it returns is positive again. */
  readonly not: Assertion<T>;
  /** The value under assertion, so a verified value can be bound, e.g.
   *  `const id = await until(() => expect(callee.header("X-Id")).toBeDefined().value())`. */
  value(): T;
  toBe(expected: T): Assertion<T>;
  toBeTruthy(): Assertion<T>;
  toBeFalsy(): Assertion<T>;
  toBeDefined(): Assertion<T>;
  toBeUndefined(): Assertion<T>;
  toBeEmpty(): Assertion<T>;
  toContain(needle: string): Assertion<T>;
  toMatch(pattern: string): Assertion<T>;
  toBeGreaterThan(n: number): Assertion<T>;
  toBeLessThan(n: number): Assertion<T>;
  toBeGreaterThanOrEqual(n: number): Assertion<T>;
  toBeLessThanOrEqual(n: number): Assertion<T>;
  /** Label this assertion (`.as("caller registered")`) — chainable, Jest has no
   *  equivalent so the name avoids colliding with a test-grouping `describe`. */
  as(label: string): Assertion<T>;
}

interface HttpResponse {
  readonly status: number;
  readonly body: string;
  header(name: string): string | undefined;
  /** The JSON value at a dotted `path` (empty for the whole body), as a native
   *  JS value. */
  json(path?: string): any;
  expectStatus(code: number): void;
}

declare class MockServer {
  constructor(opts?: { port?: number });
  /** The server's base URL (`http://127.0.0.1:<port>`), to point the SUT at. */
  readonly url: string;
  readonly port: number;
  /** Register a route: a static response object, or a per-request closure (runs on
   *  the scenario thread, pumped from `until`). `path` is a string or
   *  `regex(...)`; a leading method arg is optional. */
  respond(method: string, path: string | PathMatch, response: MockResponder): void;
  respond(path: string | PathMatch, response: MockResponder): void;
  /** How many requests arrived on `path` (string or `regex(...)`, any method) —
   *  poll via `until`. */
  requestCount(path: string | PathMatch): number;
  /** The most recent request on `path` (string or `regex(...)`) as
   *  `{ method, path, query, headers, body }`, or `undefined`. */
  lastRequest(path: string | PathMatch): MockRequestInfo | undefined;
  /** All requests on `path` (string or `regex(...)`), in arrival order. */
  requests(path: string | PathMatch): MockRequestInfo[];
  /** Stop the server early (it otherwise stops at scenario teardown). */
  stop(): void;
}

/** Set the default `until` timeout for the rest of the script (e.g. `"10s"`). */
declare function defaultTimeout(duration: string): void;
/** Read a variable: the per-file env map (`--env-file`/`<scenario>.env`/`loadEnv`)
 *  first, then the process environment; errors if unset. */
declare function env(key: string): string;
/** Begin a fluent assertion on a value. */
declare function expect<T>(actual: T): Assertion<T>;
/** A WAV-file audio source for `sendAudio`. */
declare function file(path: string): AudioSpec;
/** Performs the request off-thread and resolves with the response; `await` it.
 *  `await Promise.all([http(...), http(...)])` fires several requests concurrently. */
declare function http(method: string, url: string, opts?: HttpOptions): Promise<HttpResponse>;
/** A `application/json` response spec (body JSON-encoded) for `respond`. */
declare function jsonResponse(body: any, status?: number): MockResponseSpec;
/** Merge a dotenv file into this file's env at run time, resolved relative to the
 *  scenario's directory (later loads win). */
declare function loadEnv(path: string): void;
/** Print a timestamped note to the scenario log (and the `--json` stream). */
declare function log(msg: string): void;
/** A regex path matcher for the mock server's respond/requestCount/lastRequest/requests. */
declare function regex(pattern: string): PathMatch;
/** Register a `scenario(...)`: with a third arg, `a` is the options object and
 *  `b` the body; otherwise `a` is the body. Persists the body and records
 *  tags/skip/only from the options. */
declare function scenario(name: string, body: ScenarioBody): void;
declare function scenario(name: string, opts: ScenarioOptions, body: ScenarioBody): void;
/** Register a `setup(fn)` body, run before every scenario. Its return value becomes
 *  the per-scenario context passed to the body (and to `teardown`). */
declare function setup(body: () => any): void;
/** A silent audio source for `sendAudio`. */
declare function silence(): AudioSpec;
/** Abort the current scenario as *skipped* (reported, not failed). */
declare function skip(reason?: string): void;
/** Register a `teardown(fn)` body, run after every scenario with the context that
 *  `setup` returned. */
declare function teardown(body: (ctx: any) => void): void;
/** A `text/plain` response spec for `respond`. */
declare function textResponse(body: string, status?: number): MockResponseSpec;
/** A constant-tone audio source for `sendAudio`. */
declare function tone(freq: number): AudioSpec;
/** Resolves with `cond`'s value once it stops throwing, or rejects on timeout.
 *  `await` it (reads as `await until(...)`); the resolved value lets `.value()` bind a
 *  verified value. While waiting it yields the event loop, so several `until`/
 *  `verifyAudio` can run under `await Promise.all([...])`. */
declare function until(cond: () => unknown, within?: string): Promise<any>;
/** A fresh random UUID v4 string. */
declare function uuid(): string;
/** Assert two-way audio between two agents (a→b then b→a); resolves on success.
 *  Blocking detection runs on the runtime's blocking pool so the JS thread is free. */
declare function verifyAudioConnection(a: Agent, b: Agent): Promise<void>;
/** Hold for N seconds; rejects if a call that is established at the start drops. */
declare function wait(seconds: number): Promise<void>;
