{
  description = "Impulse - Minimalist music player with minimal dependencies, focusing on speed and a keyboard-centric TUI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # Build-time dependencies
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake
        ];

        # Runtime dependencies
        buildInputs = with pkgs; [
          alsa-lib
          libopus
        ];

        # Development tools
        devInputs = with pkgs; [
          # Pre-commit tools
          prek

          # Rust development tools
          cargo-watch
          cargo-edit
          cargo-outdated

          # Formatters and linters
          rustfmt
          clippy
        ];

      in
      {
        # Package definition
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "impulse";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = nativeBuildInputs;
          buildInputs = buildInputs;

          meta = with pkgs.lib; {
            description = "Minimalist music player with minimal dependencies, focusing on speed and a keyboard-centric TUI";
            homepage = "https://github.com/dbeley/impulse";
            license = licenses.mit;
            maintainers = [ ];
            mainProgram = "impulse";
          };
        };

        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = nativeBuildInputs ++ buildInputs ++ devInputs;

          shellHook = ''
            echo "Impulse development environment"
            echo "Available commands:"
            echo "  cargo build    - Build the project"
            echo "  cargo run      - Run the application"
            echo "  cargo test     - Run tests"
            echo "  cargo clippy   - Run linter"
            echo "  cargo fmt      - Format code"
            echo "  prek run       - Run pre-commit hooks"
            echo ""
            echo "Pre-commit hooks configured with prek"
          '';

          # Set environment variables for ALSA
          ALSA_LIB_DIR = "${pkgs.alsa-lib}/lib";
          PKG_CONFIG_PATH = "${pkgs.alsa-lib}/lib/pkgconfig";
        };

        # App definition for easy running with `nix run`
        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/impulse";
        };
      }
    ) // {
      # NixOS module for system-wide installation
      nixosModules.default = { config, lib, pkgs, ... }:
        with lib;
        let
          cfg = config.programs.impulse;
        in
        {
          options.programs.impulse = {
            enable = mkEnableOption "Impulse music player";

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.system}.default;
              description = "The impulse package to use";
            };
          };

          config = mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];
          };
        };
    };
}
