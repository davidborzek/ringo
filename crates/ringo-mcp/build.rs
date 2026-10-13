fn main() {
    let baresip_lib = std::env::var("DEP_RINGO_CORE_BARESIP_LIB");
    let re_lib = std::env::var("DEP_RINGO_CORE_RE_LIB");
    let after = std::env::var("DEP_RINGO_CORE_LINK_AFTER_ARCHIVES").unwrap_or_default();

    if let (Ok(baresip_lib), Ok(re_lib)) = (baresip_lib, re_lib) {
        // Force-include ALL symbols from libbaresip.a / libre.a so the
        // statically linked modules register via lookup_static_module().
        if cfg!(target_os = "macos") {
            println!("cargo:rustc-link-arg-bins=-Wl,-force_load,{baresip_lib}");
            println!("cargo:rustc-link-arg-bins=-Wl,-force_load,{re_lib}");
        } else {
            println!("cargo:rustc-link-arg-bins=-Wl,--whole-archive");
            println!("cargo:rustc-link-arg-bins={baresip_lib}");
            println!("cargo:rustc-link-arg-bins={re_lib}");
            println!("cargo:rustc-link-arg-bins=-Wl,--no-whole-archive");
        }
        // Resolve baresip/re's module + library deps AFTER the force-included
        // archives. ringo-core assembles the full ordered list (see its build.rs).
        for arg in after.split('\u{1f}').filter(|s| !s.is_empty()) {
            println!("cargo:rustc-link-arg-bins={arg}");
        }
    }
}
