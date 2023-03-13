# https://github.com/oxalica/rust-overlay#use-in-devshell-for-nix-develop
# https://github.com/NixOS/nixpkgs/blob/master/doc/languages-frameworks/rust.section.md#how-to-use-an-overlay-toolchain-in-a-derivation--how-to-use-an-overlay-toolchain-in-a-derivation

{
  description = "kahlo development";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-22.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
        in
          {
            devShell = import ./shell.nix { inherit pkgs; };
          }
      );
}
