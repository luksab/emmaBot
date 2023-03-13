{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  nativeBuildInputs = 
    with pkgs; [
      cargo
      pkg-config
      openssl
    ];
}
