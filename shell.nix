{ pkgs ? import <nixpkgs> {}, gui ? true }:
with pkgs;
mkShell {
  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ dbus ] ++ lib.optionals gui [ ];

  LD_LIBRARY_PATH = lib.makeLibraryPath (lib.optionals gui
    (with xorg; [ libX11 libXcursor libXrandr libXi libglvnd ]));
}
