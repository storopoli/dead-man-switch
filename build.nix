{ lib, rustPlatform, rust, buildInputs, nativeBuildInputs, package_version, ...
}:

let
  pname = "dead-man-switch-tui";
  version = package_version;

  buildRustPackage = rustPlatform.buildRustPackage.override {
    rustc = rust;
    cargo = rust;
  };

in buildRustPackage {
  inherit pname version;

  doCheck = false;

  src = ./.;

  cargoLock = { lockFile = ./Cargo.lock; };

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
