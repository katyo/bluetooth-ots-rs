{ pkgs ? import <nixpkgs> {}, gui ? true }:
with pkgs;
mkShell {
  nativeBuildInputs = [ pkgconfig ];
  buildInputs = [ dbus ] ++ lib.optionals gui [ ];

  LD_LIBRARY_PATH = lib.makeLibraryPath (lib.optionals gui
    (with xorg; [ libX11 libXcursor libXrandr libXi libglvnd ]));
}
