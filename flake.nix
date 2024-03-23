{
  description = "Rust no-BS Dead Man's Switch";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };

    };
    flake-utils.url = "github:numtide/flake-utils";

    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, pre-commit-hooks, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];

        pkgs = import nixpkgs {
          inherit system overlays;
        };

        lib = pkgs.lib;
        stdenv = pkgs.stdenv;

        isDarwin = stdenv.isDarwin;
        libsDarwin = with pkgs.darwin.apple_sdk.frameworks; lib.optionals isDarwin [
          # Additional darwin specific inputs can be set here
          Security
        ];

        msrv = pkgs.rust-bin.stable."1.63.0".default;
      in
      with pkgs;
      {
        checks = {
          pre-commit-check = pre-commit-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              typos.enable = true;

              rustfmt.enable = true;

              #clippy.enable = true;

              nixpkgs-fmt.enable = true;
            };
          };
        };

        devShells.default =
          let
            # pre-commit-checks
            _shellHook = (self.checks.${system}.pre-commit-check.shellHook or "");
          in
          mkShell {
            packages = [
              bashInteractive
              msrv
            ] ++ libsDarwin;

            shellHook = "${_shellHook}";
          };

        packages.default = import ./build.nix {
          inherit (pkgs) lib;
          rustPlatform = pkgs.rustPlatform;
          rust = msrv;
        };

        flake.overlays.default = (final: prev: {
          dead-man-switch = self.packages.${final.system}.default;
        });
      });
}
