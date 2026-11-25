{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix.url = "github:nix-community/fenix";
    utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      fenix,
      utils,
      ...
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ fenix.overlays.default ];
        };
      in
      {
        devShell = pkgs.mkShell {
          buildInputs = [
            (pkgs.fenix.complete.withComponents [
              "cargo"
              "clippy"
              "rust-src"
              "rustc"
              "rustfmt"
            ])

            pkgs.rust-analyzer-nightly
          ];

          RUST_SRC_PATH = "${pkgs.fenix.complete.rust-src}/lib/rustlib/src/rust/library";
        };

        packages.default =
          let
            manifest = pkgs.lib.importTOML ./Cargo.toml;
          in
          pkgs.rustPlatform.buildRustPackage (finalAttrs: {
            pname = manifest.package.name;
            inherit (manifest.package) version;
            src = pkgs.lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;
            meta.mainProgram = "lucida";

            nativeBuildInputs = with pkgs; [
              installShellFiles
              pkg-config
            ];

            buildInputs = [ pkgs.aws-lc ];

            postInstall =
              let
                path = "target/*/release/build/${finalAttrs.pname}-*/out";
              in
              ''
                installShellCompletion \
                  --bash ${path}/${finalAttrs.meta.mainProgram}.bash \
                  --fish ${path}/${finalAttrs.meta.mainProgram}.fish \
                  --zsh ${path}/_${finalAttrs.meta.mainProgram}
              '';
          });
      }
    );
}
