{
  rustPlatform,
  lib,
  ...
}:

let
  cargo-toml = lib.importTOML ./Cargo.toml;
  name = cargo-toml.package.name;
  pname = (builtins.elemAt cargo-toml.bin 0).name;
  version = cargo-toml.package.version;
  src = ./.;
in
rustPlatform.buildRustPackage {
  inherit name pname version src;

  cargoLock.lockFile = ./Cargo.lock;
}
