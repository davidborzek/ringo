/// A SIP account to register, independent of any ringo profile/config. Callers
/// (the softphone or the scenario runner) build this from their own source.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Account {
    pub username: String,
    pub domain: String,
    pub password: String,
    pub display_name: Option<String>,
    pub transport: Option<String>,
    pub auth_user: Option<String>,
    pub outbound: Option<String>,
    pub stun_server: Option<String>,
    pub media_enc: Option<String>,
    pub regint: Option<u32>,
    pub mwi: bool,
    /// DTMF transmission mode (`rtpevent` / `info` / `auto`). `info` sends DTMF as
    /// SIP INFO, independent of the RTP audio stream — needed where the audio TX
    /// may be idle (e.g. headless with no clocked source). `None` keeps the
    /// backend's default.
    pub dtmf_mode: Option<String>,
    /// Mark this account as a baresip `catchall` UA: incoming INVITEs whose
    /// request-URI user matches no registered contact-user fall back to this UA
    /// instead of being rejected with `404 (UA not found)`. Needed when a
    /// provider delivers calls addressed to identities other than the
    /// registration username (the request-URI user is the called number/identity,
    /// not the account login), which baresip's contact-user match would reject.
    /// Only safe with a single registered UA per process — with several, baresip
    /// routes the fallback to just one of them.
    pub catchall: bool,
}

/// Overrides for auto-detected backend settings. Any `None`/empty field
/// is auto-detected at spawn time.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BackendOptions {
    pub audio_driver: Option<String>,
    pub audio_player_device: Option<String>,
    pub audio_source_device: Option<String>,
    pub audio_alert_device: Option<String>,
    pub sip_cafile: Option<String>,
    /// `None` = auto-detect; `Some("")` = explicitly disable.
    pub sip_capath: Option<String>,
    /// Max simultaneous calls (`call_max_calls`). `None` = 4.
    pub max_calls: Option<u32>,
    /// Auto-hold the active call when another comes up / the user switches
    /// (`call_hold_other_calls`). `None` = on. The scenario runner turns this
    /// off so a test keeps explicit control over hold/resume.
    pub hold_other_calls: Option<bool>,
    /// Outgoing-call ring timeout in seconds (`call_local_timeout`). `None` = 120.
    pub local_timeout_s: Option<u32>,
    /// SIP `User-Agent` string passed to `ua_init` (e.g. `ringo-phone/0.11.0`).
    /// `None` = bare `ringo`. Set by the binary so it carries its own version.
    pub user_agent: Option<String>,
    /// Arbitrary extra config lines appended at the end (key, value).
    pub extra: Vec<(String, String)>,
    /// Capture the full call's sent + received audio in-process (for the
    /// scenario runner's `--save-audio`). When off, only a short rolling window
    /// is retained for `verify-audio`. The softphone leaves this off.
    pub record_audio: bool,
}
