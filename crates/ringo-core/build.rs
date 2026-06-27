use std::env;
use std::path::PathBuf;

fn main() {
    // Build libre + libbaresip from the bundled git submodules and link statically.
    build_vendored();

    // Generate FFI bindings from C headers via bindgen.
    generate_bindings();
}

fn build_vendored() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest.join("../..");
    let re_dir = workspace_root.join("vendor/re");
    let baresip_dir = workspace_root.join("vendor/baresip");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let profile = env::var("PROFILE").unwrap_or("debug".into());

    // ─── Locate vendored OpenSSL (built by openssl-sys) ────────────────
    //
    // openssl-sys with `vendored` builds OpenSSL from source and exposes
    // the include dir via DEP_OPENSSL_INCLUDE. We derive the lib dir and
    // pass both to cmake so libre/libbaresip compile against the same
    // static OpenSSL that gets linked into the binary.
    //
    let openssl_include = env::var("DEP_OPENSSL_INCLUDE").unwrap_or_default();
    let openssl_root: Option<PathBuf> = if !openssl_include.is_empty() {
        let root = PathBuf::from(&openssl_include)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        if root.exists() { Some(root) } else { None }
    } else {
        None
    };

    // On macOS, a build script's `cfg!(target_os/arch)` reflects the HOST, not
    // the target. For a cross-arch build (e.g. the x86_64 target on an arm64
    // runner) cmake would otherwise build libre/libbaresip for the host arch and
    // fail to link the target-arch OpenSSL ("required architecture arm64").
    // Pin cmake to the Rust target arch via CMAKE_OSX_ARCHITECTURES.
    let macos_arch: Option<&str> = if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => Some("arm64"),
            _ => Some("x86_64"),
        }
    } else {
        None
    };

    // ─── Build libre (re) ───────────────────────────────────────────────
    let re_build = out.join("re-build");
    let re_install = out.join("re-install");
    let mut re_cmake_args = vec![
        "-B".to_string(),
        re_build.to_str().unwrap().into(),
        "-S".to_string(),
        re_dir.to_str().unwrap().into(),
        format!("-DCMAKE_INSTALL_PREFIX={}", re_install.display()),
        format!(
            "-DCMAKE_BUILD_TYPE={}",
            if profile == "release" {
                "Release"
            } else {
                "Debug"
            }
        ),
        "-DBUILD_SHARED_LIBS=OFF".into(),
        "-DRE_SHARED=OFF".into(),
        "-DCMAKE_POSITION_INDEPENDENT_CODE=ON".into(),
    ];
    if let Some(ref root) = openssl_root {
        re_cmake_args.push(format!("-DOPENSSL_ROOT_DIR={}", root.display()));
        re_cmake_args.push(format!("-DOPENSSL_INCLUDE_DIR={}", openssl_include));
    }
    if let Some(arch) = macos_arch {
        re_cmake_args.push(format!("-DCMAKE_OSX_ARCHITECTURES={arch}"));
    }
    run(
        "cmake",
        &re_cmake_args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    run(
        "cmake",
        &[
            "--build",
            re_build.to_str().unwrap(),
            "--parallel",
            "--target",
            "install",
        ],
    );

    // ─── Build libbaresip (static, with selected modules) ──────────────
    //
    // Audio modules are selected by enabled_audio_modules():
    //   - default-audio: Linux pulse via pkg-config, macOS coreaudio
    //   - Explicit feature flags (pulse/alsa/coreaudio) override auto-detect
    //   - No audio feature: headless aubridge only (ringo-flow)
    // All audio modules are compiled INTO libbaresip.a (STATIC=ON) —
    // single binary, no .so files, no dlopen at runtime.
    //
    // Statically linked modules (always compiled, always in config):
    let mut static_mods = vec![
        "g711",
        "g722",
        "l16",
        "opus",
        "aubridge",
        "ausine",
        "aufile",
        "auconv",
        "auresamp",
        "stun",
        "turn",
        "ice",
        "srtp",
        "dtls_srtp",
        "mwi",
        "netroam",
    ];

    // Audio modules — from feature flags and/or platform auto-detect.
    let feature_audio_mods = enabled_audio_modules();
    static_mods.extend_from_slice(&feature_audio_mods);
    let static_modules = static_mods.join(";");

    // Pass auto-detected audio modules to Rust (config.rs reads these at
    // compile time to know which modules to list and which audio_driver
    // to default to).
    let audio_csv = feature_audio_mods.join(",");
    println!("cargo:rustc-env=RINGO_AUDIO_MODULES={audio_csv}");
    let default_audio = default_audio_driver(&feature_audio_mods);
    println!("cargo:rustc-env=RINGO_DEFAULT_AUDIO={default_audio}");

    let baresip_build = out.join("baresip-build");
    let baresip_install = out.join("baresip-install");
    let mut baresip_cmake_args = vec![
        "-B".to_string(),
        baresip_build.to_str().unwrap().into(),
        "-S".to_string(),
        baresip_dir.to_str().unwrap().into(),
        format!("-DCMAKE_INSTALL_PREFIX={}", baresip_install.display()),
        format!(
            "-DCMAKE_BUILD_TYPE={}",
            if profile == "release" {
                "Release"
            } else {
                "Debug"
            }
        ),
        format!("-DMODULES={}", static_modules),
        "-DSTATIC=ON".into(),
        "-DCMAKE_POSITION_INDEPENDENT_CODE=ON".into(),
        format!("-Dre_DIR={}/lib/cmake/re", re_install.display()),
        format!("-DCMAKE_PREFIX_PATH={}", re_install.display()),
    ];
    if let Some(ref root) = openssl_root {
        baresip_cmake_args.push(format!("-DOPENSSL_ROOT_DIR={}", root.display()));
        baresip_cmake_args.push(format!("-DOPENSSL_INCLUDE_DIR={}", openssl_include));
    }
    if let Some(arch) = macos_arch {
        baresip_cmake_args.push(format!("-DCMAKE_OSX_ARCHITECTURES={arch}"));
    }
    run(
        "cmake",
        &baresip_cmake_args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
    );
    run(
        "cmake",
        &[
            "--build",
            baresip_build.to_str().unwrap(),
            "--parallel",
            "--target",
            "install",
        ],
    );

    // ─── Link system libraries for statically-linked audio modules ────
    //
    // When audio features are enabled (or auto-detected), the module is
    // compiled into libbaresip.a (STATIC=ON). The module's C code calls
    // functions from the system audio library (e.g. pa_* from libpulse).
    // We must link those system libraries so the symbols resolve at link time.
    //
    link_audio_system_libs(&feature_audio_mods);

    // ─── Link statically ───────────────────────────────────────────────
    // Use --whole-archive for libbaresip + libre so ALL symbols are
    // included, not just those referenced by Rust code.

    println!(
        "cargo:rustc-link-search=native={}",
        baresip_install.join("lib").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        re_install.join("lib").display()
    );
    // Expose .a paths to downstream build scripts via DEP_RINGO_CORE_*.
    println!(
        "cargo:baresip_lib={}",
        baresip_install.join("lib/libbaresip.a").display()
    );
    println!("cargo:re_lib={}", re_install.join("lib/libre.a").display());

    // System libs that baresip/re and the statically linked modules depend on.
    //
    // OpenSSL (ssl/crypto) — vendored by openssl-sys, statically linked.
    // spandsp + opus       — system shared libs (stable SONAME, no ABI break).
    //
    //   z → zlib (compression)
    //
    for lib in &["z", "resolv", "m", "dl", "pthread"] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
    // On macOS, Homebrew installs spandsp/opus outside the default linker
    // search path, so `-lspandsp`/`-lopus` would not resolve. Add the brew
    // prefix's lib dir. Linux keeps them on the default path (/usr/lib).
    if cfg!(target_os = "macos") {
        if let Ok(out) = std::process::Command::new("brew").arg("--prefix").output() {
            if let Ok(prefix) = String::from_utf8(out.stdout) {
                println!("cargo:rustc-link-search=native={}/lib", prefix.trim());
            }
        }
    }
    println!("cargo:rustc-link-lib=dylib=spandsp");
    println!("cargo:rustc-link-lib=dylib=opus");

    // Link vendored OpenSSL (static). openssl-sys builds it from source
    // and we point cmake at the same install prefix. We must explicitly
    // link the static libs here so symbols referenced by libre/libbaresip
    // (SSL_get_error, SSL_CTX_*, etc.) resolve at link time.
    if let Some(ref root) = openssl_root {
        let lib_dir = root.join("lib");
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=ssl");
        println!("cargo:rustc-link-lib=static=crypto");
    }

    // ─── Link sequence for the binary crates (ringo-phone / ringo-flow) ───
    //
    // libbaresip.a + libre.a are force-included (--whole-archive / -force_load)
    // at the END of each bin's link line, so EVERY symbol their statically
    // linked modules reference must resolve AFTER them: g722 → spandsp, codecs
    // → opus, the audio driver → libpulse/etc, TLS → OpenSSL, and libre →
    // zlib/resolver/(SystemConfiguration on macOS). The bin crates can't know
    // this set, so build the full ordered list here and hand it over via
    // DEP_RINGO_CORE_LINK_AFTER_ARCHIVES. (x86-Linux tolerates the wrong order;
    // macOS ld and the aarch64 linker do not.)
    let mut after: Vec<String> = Vec::new();
    after.push("-lspandsp".into());
    after.push("-lopus".into());
    for m in &feature_audio_mods {
        match *m {
            "pulse" => {
                after.push("-lpulse".into());
                after.push("-lpulse-simple".into());
            }
            "alsa" => after.push("-lasound".into()),
            "coreaudio" => {
                after.push("-framework".into());
                after.push("CoreAudio".into());
                after.push("-framework".into());
                after.push("AudioToolbox".into());
            }
            _ => {}
        }
    }
    if let Some(ref root) = openssl_root {
        after.push(format!("{}/lib/libssl.a", root.display()));
        after.push(format!("{}/lib/libcrypto.a", root.display()));
    }
    if cfg!(target_os = "macos") {
        after.push("-framework".into());
        after.push("SystemConfiguration".into());
        after.push("-lz".into());
        after.push("-lresolv".into());
    } else {
        after.push("-lz".into());
        after.push("-lresolv".into());
        after.push("-ldl".into());
        after.push("-lpthread".into());
        after.push("-lc".into());
        after.push("-lm".into());
    }
    // Separated by US (\x1f), not space: survives paths containing spaces (a
    // macOS OUT_DIR under "/Users/Some Name/…") AND can't be a newline, which
    // would truncate this line-oriented `cargo:` directive.
    println!("cargo:link_after_archives={}", after.join("\u{1f}"));

    // Tell Rust that the build depends on the vendored sources. Absolute paths
    // (workspace root), since these live at <root>/vendor, not under the crate dir.
    println!(
        "cargo:rerun-if-changed={}",
        workspace_root.join("vendor/re/src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        workspace_root.join("vendor/baresip/src").display()
    );
}

fn enabled_audio_modules() -> Vec<&'static str> {
    let mut mods = vec![];
    if cfg!(feature = "pulse") {
        mods.push("pulse");
    }
    if cfg!(feature = "alsa") {
        mods.push("alsa");
    }
    if cfg!(feature = "coreaudio") {
        mods.push("coreaudio");
    }

    // No explicit audio features plus default-audio → auto-detect, unless
    // RINGO_NO_AUDIO is set as an escape hatch for packaging.
    if mods.is_empty() && cfg!(feature = "default-audio") && env::var("RINGO_NO_AUDIO").is_err() {
        #[cfg(target_os = "macos")]
        {
            mods.push("coreaudio");
        }
        #[cfg(target_os = "linux")]
        {
            if pkg_config::probe_library("libpulse").is_ok() {
                mods.push("pulse");
            }
        }
    }

    mods
}

/// Pick the default audio_driver for config.rs:
/// - If audio modules were compiled, use the first one (pulse > alsa > coreaudio).
/// - If none, "aubridge" (headless virtual loopback).
fn default_audio_driver(mods: &[&str]) -> String {
    for pref in &["pulse", "alsa", "coreaudio"] {
        if mods.contains(pref) {
            return pref.to_string();
        }
    }
    "aubridge".to_string()
}

/// Link system audio libraries (libpulse, libasound, CoreAudio)
/// for statically-linked audio modules. Uses pkg-config on Linux; on macOS
/// CoreAudio is a system framework.
fn link_audio_system_libs(mods: &[&str]) {
    for m in mods {
        match *m {
            "pulse" => {
                if let Ok(lib) = pkg_config::probe_library("libpulse") {
                    for path in &lib.include_paths {
                        println!("cargo:rustc-link-search=native={}", path.display());
                    }
                    println!("cargo:rustc-link-lib=pulse");
                }
                if let Ok(lib) = pkg_config::probe_library("libpulse-simple") {
                    for path in &lib.include_paths {
                        println!("cargo:rustc-link-search=native={}", path.display());
                    }
                    println!("cargo:rustc-link-lib=pulse-simple");
                }
            }
            "alsa" => {
                if let Ok(lib) = pkg_config::probe_library("alsa") {
                    for path in &lib.include_paths {
                        println!("cargo:rustc-link-search=native={}", path.display());
                    }
                    println!("cargo:rustc-link-lib=asound");
                }
            }
            "coreaudio" => {
                println!("cargo:rustc-link-lib=framework=CoreAudio");
                println!("cargo:rustc-link-lib=framework=AudioToolbox");
            }
            _ => {}
        }
    }
}

fn run(cmd: &str, args: &[&str]) {
    use std::process::Command;
    let status = Command::new(cmd)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to run {cmd}: {e}"));
    if !status.success() {
        panic!("{cmd} failed with status {status}");
    }
}

/// Generate Rust FFI bindings from the libre + libbaresip C headers using bindgen.
/// This replaces hand-written `extern "C"` declarations — the signatures are
/// verified against the actual headers at build time.
fn generate_bindings() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let wrapper = manifest.join("wrapper.h");

    // Include paths: vendored headers + build output (for generated config headers).
    let mut include_paths: Vec<PathBuf> = Vec::new();

    let workspace_root = manifest.join("../..");
    include_paths.push(workspace_root.join("vendor/re/include"));
    include_paths.push(workspace_root.join("vendor/baresip/include"));
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    include_paths.push(out.join("re-install/include"));
    include_paths.push(out.join("baresip-install/include"));

    let mut builder = bindgen::Builder::default()
        .header(wrapper.to_str().unwrap())
        .rust_edition(bindgen::RustEdition::Edition2024)
        .size_t_is_usize(true)
        .allowlist_function("libre_init|libre_close|re_main|re_cancel|re_thread.*|re_thread_async_close|dbg_.*|mem_deref|mod_close|list_apply")
        .allowlist_function("ua_.*|uag_find_msg|uag_list|uag_call_count|baresip_init|baresip_close|conf_.*|module_load|module_app_unload|module_unload")
        .allowlist_function("account_.*|call_.*|audio_.*")
        .allowlist_function("ausrc_register|baresip_ausrcl|auplay_register|baresip_auplayl|auframe_init|aufmt_sample_size|mem_zalloc|mem_alloc")
        .allowlist_function("bevent_.*")
        .allowlist_function("uag_sip|sip_set_trace_handler|sip_transp_name|sip_treplyf")
        .allowlist_function("sa_af|sa_in|sa_in6|sa_port")
        .allowlist_function("log_enable_.*|log_register_handler|log_unregister_handler|log_level_set|log_level_get")
        .allowlist_function("play_.*|baresip_player|play_set_.*")
        .allowlist_function("mbuf_.*")
        .allowlist_function("g711_.*")
        .allowlist_type("ua|call|bevent|audio|account|config|list|le|pl|sip_hdr|sip_msg|sip_taddr|list_apply_h")
        .allowlist_type("vidmode|dtmfmode|bevent_ev|dbg_flags|dbg_print_h|log_level|log_h|log")
        .allowlist_type("play|player|play_finish_h")
        .allowlist_type("mbuf")
        .allowlist_type("ausrc|ausrc_prm|ausrc_read_h|ausrc_error_h|ausrc_alloc_h|auframe|aufmt|mem_destroy_h")
        .allowlist_type("auplay|auplay_prm|auplay_write_h|auplay_alloc_h")
        .allowlist_type("sip|sa|sip_transp|sip_trace_h|sip_strans")
        .allowlist_var("KEYCODE_REL")
        .opaque_type("ua|call|bevent|audio|account|config")
        .opaque_type("play|player")
        .opaque_type("sip|sa|sip_strans")
        .rustified_enum("vidmode|dtmfmode|bevent_ev|aufmt")
        .derive_default(false)
        .derive_copy(true)
        .generate_comments(false)
        .layout_tests(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for path in &include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    let bindings = builder
        .generate()
        .expect("failed to generate bindings from C headers");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("failed to write bindings.rs");
}
