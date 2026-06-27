use std::ffi::CString;

use crate::account::{Account, BackendOptions};

use super::bindings::*;

/// Build a baresip config string from account + options, for `conf_configure_buf`.
// A long, partly-conditional list builder — vec![] init would be unreadable here.
#[allow(clippy::vec_init_then_push)]
pub fn build_config_string(account: &Account, options: &BackendOptions) -> String {
    let mut lines = Vec::new();

    // Modules — all statically linked into libbaresip.a, so module_load()
    // resolves them via lookup_static_module() without dlopen. The `.so`
    // suffix is kept because it's the canonical baresip module name;
    // lookup_static_module() strips it.
    //
    // Codecs (always needed for SDP negotiation):
    lines.push("module                  g711.so".to_string());
    lines.push("module                  g722.so".to_string());
    lines.push("module                  opus.so".to_string());
    lines.push("module                  l16.so".to_string());
    // No uuid module: it would write a per-process `uuid` file into conf_path
    // just to set +sip.instance (GRUU/RFC5626). Our conf_path is an ephemeral
    // temp dir, so the UUID changes every run — no stable-instance benefit, only
    // /tmp clutter. baresip omits +sip.instance when the uuid is empty. (A stable
    // instance id for ringo-phone would be persisted in the config dir, later.)
    // Audio player for headless: aubridge (virtual loopback). The audio SOURCE
    // is ringo's own module (registered in code, see ausrc.rs) — tone/file/
    // silence are rendered in Rust, so baresip's ausine/aufile aren't needed.
    lines.push("module                  aubridge.so".to_string());
    // Audio filters (same as old process backend):
    lines.push("module                  auconv.so".to_string());
    lines.push("module                  auresamp.so".to_string());
    // No sndfile module: ringo captures sent/received audio in-process via its
    // own ausrc/auplay (see ausrc.rs), so verify-audio and --save-audio need no
    // WAV dumps on disk. `record_audio` toggles full-call in-process capture.
    // NAT traversal + media encryption:
    lines.push("module                  stun.so".to_string());
    lines.push("module                  turn.so".to_string());
    lines.push("module                  ice.so".to_string());
    lines.push("module                  srtp.so".to_string());
    lines.push("module                  dtls_srtp.so".to_string());
    // MWI — message waiting indication (SUBSCRIBE/NOTIFY for voicemail):
    lines.push("module                  mwi.so".to_string());
    // Network roaming — re-registers on network change (WiFi↔Ethernet):
    lines.push("module                  netroam.so".to_string());

    // Audio driver modules — compiled into libbaresip.a by build.rs via explicit
    // features or default-audio auto-detect. Headless builds leave this empty and
    // use aubridge only.
    let audio_modules: Vec<&str> = env!("RINGO_AUDIO_MODULES", "")
        .split(',')
        .filter(|s| !s.is_empty())
        .collect();
    for m in &audio_modules {
        lines.push(format!("module                  {m}.so"));
    }

    // Default audio driver — selected by build.rs, overridable at runtime.
    let default_driver = env!("RINGO_DEFAULT_AUDIO", "aubridge");
    let audio_driver = options.audio_driver.as_deref().unwrap_or(default_driver);
    lines.push(format!("audio_driver            {audio_driver}"));
    lines.push(format!("audio_player            {audio_driver},default"));
    lines.push(format!("audio_source            {audio_driver},default"));
    lines.push(format!("audio_alert             {audio_driver},default"));
    // Audio format + buffer settings (same as old process backend):
    lines.push("ausrc_format            s16".to_string());
    lines.push("auplay_format           s16".to_string());
    lines.push("auenc_format            s16".to_string());
    lines.push("audec_format            s16".to_string());
    lines.push("audio_buffer            20-160".to_string());
    lines.push("audio_buffer_mode       fixed".to_string());
    lines.push("audio_telev_pt          101".to_string());
    lines.push("audio_jitter_buffer_type fixed".to_string());
    lines.push("audio_jitter_buffer_ms    100-200".to_string());
    lines.push("audio_jitter_buffer_size  50".to_string());

    // Call settings (same as old process backend):
    lines.push("call_local_timeout  120".to_string());
    lines.push("call_max_calls      4".to_string());
    lines.push("call_hold_other_calls yes".to_string());

    // QoS markings (same as old process backend):
    lines.push("sip_tos             160".to_string());
    lines.push("rtp_tos             184".to_string());

    // call_accept=no: we handle BEVENT_SIPSESS_CONN ourselves (extract ALL
    // SIP headers from the INVITE message, then call ua_accept).
    lines.push("call_accept             no".to_string());

    // SIP TLS
    if let Some(ref cafile) = options.sip_cafile {
        lines.push(format!("sip_cafile              {cafile}"));
    }
    if let Some(ref capath) = options.sip_capath {
        if !capath.is_empty() {
            lines.push(format!("sip_capath              {capath}"));
        }
    }

    // MWI
    if account.mwi {
        lines.push("mwi                     yes".to_string());
    }

    // Extra config lines (e.g. "module ausine.so", "module aufile.so")
    for (k, v) in &options.extra {
        lines.push(format!("{k:<24}{v}"));
    }

    lines.join("\n")
}

/// Convert a string to a `CString`, mapping an interior NUL byte to an error.
/// The offending value is NOT included in the message — account fields carry
/// credentials that must never leak into logs.
fn cstr(value: &str, field: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("account field `{field}` contains a NUL byte"))
}

/// Configure a baresip account from ringo's neutral `Account` struct.
/// Must run on the RE thread. Returns `Err` if a field contains an interior
/// NUL byte (rejected rather than panicking on hostile input).
pub fn configure_account(acc: *mut AccountC, account: &Account) -> Result<(), String> {
    unsafe {
        let user = cstr(&account.username, "username")?;
        account_set_auth_user(acc, user.as_ptr());

        let pass = cstr(&account.password, "password")?;
        account_set_auth_pass(acc, pass.as_ptr());

        if let Some(ref auth_user) = account.auth_user {
            if !auth_user.is_empty() {
                let au = cstr(auth_user, "auth_user")?;
                account_set_auth_user(acc, au.as_ptr());
            }
        }

        if let Some(ref display) = account.display_name {
            if !display.is_empty() {
                let d = cstr(display, "display_name")?;
                account_set_display_name(acc, d.as_ptr());
            }
        }

        // Outbound proxy — if not set, use the domain as default outbound
        // (baresip's account module does this when parsing accounts files,
        // but we bypass it with ua_alloc, so we need to set it manually).
        let outbound = account
            .outbound
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("sip:{}", account.domain));
        let ob = cstr(&outbound, "outbound")?;
        account_set_outbound(acc, ob.as_ptr(), 0);

        // STUN server — use account_set_stun_uri (not account_set_stun_host)
        // because we need to parse the full URI (e.g. "stun:stun.example.com").
        if let Some(ref stun) = account.stun_server {
            if !stun.is_empty() {
                let s = cstr(stun, "stun_server")?;
                let rc = account_set_stun_uri(acc, s.as_ptr());
                if rc != 0 {
                    crate::rlog!(Warn, "account_set_stun_uri() failed (rc={rc})");
                }
            }
        }

        if let Some(ref mediaenc) = account.media_enc {
            if !mediaenc.is_empty() {
                let m = cstr(mediaenc, "media_enc")?;
                account_set_mediaenc(acc, m.as_ptr());
            }
        }

        let regint = account.regint.unwrap_or(3600);
        account_set_regint(acc, regint);

        account_set_mwi(acc, account.mwi);

        if let Some(ref dtmf) = account.dtmf_mode {
            let mode = match dtmf.as_str() {
                "info" => dtmfmode::DTMFMODE_SIP_INFO,
                "rtpevent" => dtmfmode::DTMFMODE_RTP_EVENT,
                "auto" => dtmfmode::DTMFMODE_AUTO,
                _ => dtmfmode::DTMFMODE_RTP_EVENT,
            };
            account_set_dtmfmode(acc, mode);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Account {
        Account {
            username: "alice".into(),
            domain: "example.com".into(),
            password: "secret".into(),
            ..Default::default()
        }
    }

    /// True if the config has a line whose first whitespace-separated token is
    /// `key` and second is `val`. Whitespace-tolerant so the fixed column
    /// alignment in `build_config_string` can change without breaking tests.
    fn has_directive(config: &str, key: &str, val: &str) -> bool {
        config.lines().any(|l| {
            let mut it = l.split_whitespace();
            it.next() == Some(key) && it.next() == Some(val)
        })
    }

    fn has_key(config: &str, key: &str) -> bool {
        config
            .lines()
            .any(|l| l.split_whitespace().next() == Some(key))
    }

    #[test]
    fn codecs_and_core_modules_always_present() {
        let cfg = build_config_string(&account(), &BackendOptions::default());
        for module in [
            "g711.so",
            "g722.so",
            "opus.so",
            "l16.so",
            "stun.so",
            "turn.so",
            "ice.so",
            "srtp.so",
            "dtls_srtp.so",
            "mwi.so",
            "netroam.so",
        ] {
            assert!(
                has_directive(&cfg, "module", module),
                "missing module {module}"
            );
        }
    }

    #[test]
    fn call_accept_is_disabled_for_manual_invite_handling() {
        // The SIPSESS_CONN header-extraction path depends on baresip NOT
        // auto-accepting — regressing this silently breaks inbound headers.
        let cfg = build_config_string(&account(), &BackendOptions::default());
        assert!(has_directive(&cfg, "call_accept", "no"));
    }

    #[test]
    fn no_sndfile_module_regardless_of_record_audio() {
        // Audio is captured in-process (ausrc.rs), never via baresip's sndfile
        // module — so neither it nor snd_path should ever appear in the config.
        for record_audio in [false, true] {
            let opts = BackendOptions {
                record_audio,
                ..Default::default()
            };
            let cfg = build_config_string(&account(), &opts);
            assert!(!has_directive(&cfg, "module", "sndfile.so"));
            assert!(!has_key(&cfg, "snd_path"));
        }
    }

    #[test]
    fn explicit_audio_driver_overrides_default() {
        let opts = BackendOptions {
            audio_driver: Some("aubridge".into()),
            ..Default::default()
        };
        let cfg = build_config_string(&account(), &opts);
        assert!(has_directive(&cfg, "audio_driver", "aubridge"));
        assert!(has_directive(&cfg, "audio_player", "aubridge,default"));
        assert!(has_directive(&cfg, "audio_source", "aubridge,default"));
        assert!(has_directive(&cfg, "audio_alert", "aubridge,default"));
    }

    #[test]
    fn tls_cafile_and_capath_handling() {
        // cafile set, capath auto-detect (None) → only cafile emitted.
        let opts = BackendOptions {
            sip_cafile: Some("/etc/ssl/ca.pem".into()),
            ..Default::default()
        };
        let cfg = build_config_string(&account(), &opts);
        assert!(has_directive(&cfg, "sip_cafile", "/etc/ssl/ca.pem"));
        assert!(!has_key(&cfg, "sip_capath"));

        // capath explicitly disabled with Some("") → no capath line.
        let opts = BackendOptions {
            sip_capath: Some(String::new()),
            ..Default::default()
        };
        let cfg = build_config_string(&account(), &opts);
        assert!(!has_key(&cfg, "sip_capath"));

        // capath set → emitted.
        let opts = BackendOptions {
            sip_capath: Some("/etc/ssl/certs".into()),
            ..Default::default()
        };
        let cfg = build_config_string(&account(), &opts);
        assert!(has_directive(&cfg, "sip_capath", "/etc/ssl/certs"));
    }

    #[test]
    fn mwi_enabled_for_account() {
        let mut acc = account();
        acc.mwi = true;
        let cfg = build_config_string(&acc, &BackendOptions::default());
        assert!(has_directive(&cfg, "mwi", "yes"));
    }

    #[test]
    fn cstr_rejects_nul_without_leaking_the_value() {
        let err = cstr("se\0cret", "password").unwrap_err();
        assert!(err.contains("password"), "error should name the field");
        assert!(
            !err.contains("se") && !err.contains("cret"),
            "credential value must never appear in the error"
        );
        assert!(cstr("clean-value", "username").is_ok());
    }

    #[test]
    fn extra_lines_are_appended() {
        let opts = BackendOptions {
            extra: vec![("module".into(), "aufile.so".into())],
            ..Default::default()
        };
        let cfg = build_config_string(&account(), &opts);
        assert!(has_directive(&cfg, "module", "aufile.so"));
    }
}
