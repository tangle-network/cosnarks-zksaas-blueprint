{
  description = "Hello World Blueprint development environment";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    # Rust
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    foundry = {
      url = "github:shazow/foundry.nix/stable";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      foundry,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [
          (import rust-overlay)
          foundry.overlay
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        lib = pkgs.lib;
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          name = "blueprint";
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.clang
            pkgs.libclang.lib
            pkgs.openssl.dev
            pkgs.gmp
            # Mold Linker for faster builds (only on Linux)
            (lib.optionals pkgs.stdenv.isLinux pkgs.mold)
            (lib.optionals pkgs.stdenv.isDarwin pkgs.darwin.apple_sdk.frameworks.Security)
            (lib.optionals pkgs.stdenv.isDarwin pkgs.darwin.apple_sdk.frameworks.SystemConfiguration)
          ];
          buildInputs = [
            # We want the unwrapped version, wrapped comes with nixpkgs' toolchain
            pkgs.rust-analyzer-unwrapped
            # Finally the toolchain
            toolchain
            pkgs.foundry-bin
            pkgs.taplo

            pkgs.cargo-nextest
            pkgs.cargo-expand
          ];

          packages = [ ];
          # Environment variables
          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
          LD_LIBRARY_PATH = lib.makeLibraryPath [
            pkgs.gmp
            pkgs.libclang
            pkgs.openssl.dev
            pkgs.stdenv.cc.cc
          ];
          # Add Forge from foundry to $HOME/.config/.foundry/bin/forge
          # by symlinking it to the flake's bin
          shellHook = ''
            mkdir -p $HOME/.config/.foundry/bin
            ln -s ${pkgs.foundry-bin}/bin/forge $HOME/.config/.foundry/bin/forge
          '';
        };
      }
    );
}
