{
  description = "ringo — terminal SIP softphone (ringo-phone) and scenario runner (ringo-flow)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # libre + libbaresip are git submodules of ringo-core (see .gitmodules).
    # Flake sources do not carry submodules, so we pin the same upstream release
    # tags here and copy them into crates/ringo-core/vendor/ before the build.
    # Keep these tags in sync with `branch = ` in .gitmodules (and re-run
    # `nix flake update re baresip` after bumping).
    re = {
      url = "github:baresip/re/v4.10.0";
      flake = false;
    };
    baresip = {
      url = "github:baresip/baresip/v4.10.0";
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
          '';

          nativeBuildInputs = [
            pkgs.cmake # builds libre + libbaresip
            pkgs.pkg-config # locates opus/spandsp/libpulse
            pkgs.perl # openssl-sys vendored build
            pkgs.rustPlatform.bindgenHook # sets LIBCLANG_PATH + clang headers for bindgen
            pkgs.installShellFiles # ship shell completions
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

          # Generate + install shell completions from the built binary
          # (clap's COMPLETE=<shell> emits the script). Skipped on cross builds.
          postInstall = lib.optionalString (pkgs.stdenv.buildPlatform.canExecute pkgs.stdenv.hostPlatform) ''
            installShellCompletion --cmd ${bin} \
              --fish <(COMPLETE=fish $out/bin/${bin}) \
              --bash <(COMPLETE=bash $out/bin/${bin}) \
              --zsh <(COMPLETE=zsh $out/bin/${bin})
          '';

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
