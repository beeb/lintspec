{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ fenix.overlays.default ];
        };
        lib = pkgs.lib;
        toolchain = fenix.packages.${system}.fromToolchainFile { dir = ./.; };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.rust-analyzer-unwrapped
            toolchain
            pkgs.cargo-insta
            pkgs.cargo-nextest
            pkgs.cargo-dist
          ];

          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "lintspec";
          inherit ((lib.importTOML ./Cargo.toml).package) version;

          src = lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          doCheck = false;
        };
      });
}
