{
  description = "ringo — terminal SIP softphone (ringo-phone) and scenario runner (ringo-flow)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # libre + libbaresip are git submodules of ringo-core (see .gitmodules).
    # Flake sources do not carry submodules, so we pin the exact submodule
    # commits here and copy them into crates/ringo-core/vendor/ before the
    # build. Keep these revs in sync with `git submodule status`.
    re = {
      url = "github:baresip/re/e11a4c584bdb0cb30dde9d4f3e8a7a5717506855";
      flake = false;
    };
    baresip = {
      url = "github:baresip/baresip/c06e3678cab6e7b5cfb504101cb82f862f2efb15";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      re,
      baresip,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        # Apple Silicon only; nixpkgs-unstable dropped x86_64-darwin (Intel).
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      pkgsFor = system: nixpkgs.legacyPackages.${system};

      # Shared derivation builder. `crate` is the workspace member / cargo
      # package; `bin` is the produced executable; `audio` toggles the
      # default-audio (PulseAudio) backend — on for the phone, off (headless
      # aubridge) for flow.
      mkRingo =
        {
          pkgs,
          crate,
          bin,
          audio,
        }:
        let
          lib = pkgs.lib;
          cargoToml = lib.importTOML (./crates + "/${crate}/Cargo.toml");
          # ringo-core's build.rs hardcodes the vendored install path as
          # `<out>/lib/lib{re,baresip}.a`, but GNUInstallDirs picks `lib64` in
          # the Nix sandbox (no distro marker under /etc). This shim forces
          # `CMAKE_INSTALL_LIBDIR=lib` on the configure step (not on
          # `cmake --build`, which rejects -D flags).
          cmakeShim = pkgs.writeShellScript "cmake" ''
            for a in "$@"; do
              [ "$a" = "--build" ] && exec ${pkgs.cmake}/bin/cmake "$@"
            done
            exec ${pkgs.cmake}/bin/cmake "$@" -DCMAKE_INSTALL_LIBDIR=lib
          '';
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = crate;
          version = cargoToml.package.version;

          src = self;
          cargoLock.lockFile = ./Cargo.lock;

          # Inject the vendored C sources (git submodules) that the flake
          # source does not include.
          postPatch = ''
            mkdir -p crates/ringo-core/vendor
            rm -rf crates/ringo-core/vendor/re crates/ringo-core/vendor/baresip
            cp -r --no-preserve=mode,ownership ${re} crates/ringo-core/vendor/re
            cp -r --no-preserve=mode,ownership ${baresip} crates/ringo-core/vendor/baresip

            # Shadow `cmake` with the lib-dir-forcing shim (see cmakeShim).
            mkdir -p "$TMPDIR/cmake-shim"
            ln -sf ${cmakeShim} "$TMPDIR/cmake-shim/cmake"
            export PATH="$TMPDIR/cmake-shim:$PATH"
          '';

          nativeBuildInputs = [
            pkgs.cmake # builds libre + libbaresip
            pkgs.pkg-config # locates opus/spandsp/libpulse
            pkgs.perl # openssl-sys vendored build
            pkgs.rustPlatform.bindgenHook # sets LIBCLANG_PATH + clang headers for bindgen
          ];

          # Dynamically linked libs — placed in buildInputs so the nix
          # cc-wrapper adds their store paths to the binary's RPATH. OpenSSL is
          # vendored (statically linked), so it is intentionally absent here.
          #
          # Audio backend for the phone (`audio = true`): PulseAudio on Linux
          # (build.rs auto-detects it via pkg-config), CoreAudio on macOS. The
          # macOS CoreAudio/AudioToolbox/SystemConfiguration frameworks come
          # from the Apple SDK in the darwin stdenv — no derivation needed.
          buildInputs =
            [
              pkgs.opus
              pkgs.spandsp
              pkgs.zlib
            ]
            ++ lib.optional (audio && pkgs.stdenv.isLinux) pkgs.libpulseaudio;

          # cmake is only used by build.rs to compile the vendored C libs; the
          # setup hook's configure/build phases would fight cargo, so disable it.
          dontUseCmakeConfigure = true;

          cargoBuildFlags = [
            "-p"
            crate
          ];

          # Headless build for ringo-flow: no default-audio feature.
          buildNoDefaultFeatures = !audio;

          # The test suites are not part of the install artifact and some need a
          # SIP peer / audio device; build the binary only.
          doCheck = false;

          meta = {
            inherit (cargoToml.package) description;
            homepage = "https://github.com/davidborzek/ringo";
            license = lib.licenses.mit;
            mainProgram = bin;
            platforms = systems;
          };
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        rec {
          ringo-phone = mkRingo {
            inherit pkgs;
            crate = "ringo-phone";
            bin = "ringo";
            audio = true;
          };
          ringo-flow = mkRingo {
            inherit pkgs;
            crate = "ringo-flow";
            bin = "ringo-flow";
            audio = false;
          };
          default = ringo-phone;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          # Inherit the full build environment (cmake, pkg-config, perl, bindgen,
          # opus, spandsp, zlib, libpulseaudio, cargo, rustc) from the package so
          # the shell can never drift from the actual build.
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.ringo-phone ];
            packages = with pkgs; [ rustfmt clippy rust-analyzer ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        }
      );

      overlays.default = final: _prev: {
        ringo-phone = self.packages.${final.stdenv.hostPlatform.system}.ringo-phone;
        ringo-flow = self.packages.${final.stdenv.hostPlatform.system}.ringo-flow;
      };

      nixosModules = rec {
        ringo-flow = import ./nix/nixos-ringo-flow.nix { inherit self; };
        default = ringo-flow;
      };

      homeManagerModules = rec {
        ringo = import ./nix/hm-ringo.nix { inherit self; };
        default = ringo;
      };
    };
}
