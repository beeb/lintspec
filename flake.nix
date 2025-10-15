{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix }:
    let
      forAllSystems = nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ fenix.overlays.default ];
          };
          toolchain = fenix.packages.${system}.fromToolchainFile { dir = ./.; };
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo-dist
              cargo-hack
              cargo-insta
              cargo-nextest
              toolchain
            ];

            RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
          };
        }
      );
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          lib = pkgs.lib;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "lintspec";
            inherit ((lib.importTOML ./crates/lintspec/Cargo.toml).package) version;

            src = lib.cleanSource ./.;
            cargoBuildFlags = [ "--package" "lintspec" ];

            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };

            nativeBuildInputs = [ pkgs.installShellFiles ];
            postInstall = lib.optionalString (pkgs.stdenv.buildPlatform.canExecute pkgs.stdenv.hostPlatform) ''
              installShellCompletion --cmd lintspec \
                --bash <($out/bin/lintspec completions -s bash) \
                --fish <($out/bin/lintspec completions -s fish) \
                --zsh <($out/bin/lintspec completions -s zsh)
            '';

            doCheck = false;
          };
        }
      );
    };
}
