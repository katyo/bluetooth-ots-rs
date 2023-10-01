{ pkgs ? import <nixpkgs> {} }:
with pkgs;
mkShell {
  nativeBuildInputs = [ pkgconfig ];
  buildInputs = [ dbus ];
}
