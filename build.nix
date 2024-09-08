{ lib, rustPlatform, rust, buildInputs, nativeBuildInputs, ... }:

let
  pname = "dead-man-switch-tui";
  version = "0.4.0";

  buildRustPackage = rustPlatform.buildRustPackage.override {
    rustc = rust;
    cargo = rust;
  };
in

buildRustPackage {
  inherit pname version;

  doCheck = false;

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  inherit buildInputs nativeBuildInputs;

  # Override the Rust compiler used
  rustc = "${rust}/bin/rustc";
  cargo = "${rust}/bin/cargo";

  meta = with lib; {
    description = "Rust no-BS Dead Man's Switch";
    homepage = "https://github.com/storopoli/dead-man-switch";
    license = licenses.agpl3Only;
    maintainers = [ maintainers.storopoli ];
    platforms = platforms.all;
  };
}
