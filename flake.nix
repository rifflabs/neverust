{
  description = "Base flake for the neverust project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell {
          buildInputs = [
            rust-bin.stable.latest.default
            openssl
            pkg-config
            libclang.lib
            # Used to cache C compilation artifacts. Particularly useful in a project using librocksdb-sys
            sccache
          ];
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";

          shellHook = ''
            echo "in rust dev shell";
            export SCCACHE_DIR="$HOME/.cache/sccache"
            mkdir -p $SCCACHE_DIR
          '';
        };
      }
    );
}

