{
  description = "Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShell = mkShell rec {
          inputsFrom = with pkgs; [
            gtk3
            webkitgtk_4_1
            libsoup_3
          ];

          buildInputs = with pkgs; [
            (rust-bin.stable.latest.default.override {
              extensions = [
                "rust-analyzer"
              ];
              targets = [
                "thumbv8m.main-none-eabihf"
              ];
            })
            cargo-watch
            cargo-edit

            picotool
            bun

            # Slint (winit + software renderer) on Linux
            fontconfig
            expat
            libGL
            wayland
            libXcursor
            libXrandr
            libXi
            libX11
            libXext
            libXfixes
            libxcb
            libxkbcommon

            # Tauri v2 (Linux GTK/WebKit backend)
            glib
            gtk3
            webkitgtk_4_1
            libsoup_3
            cairo
            pango
            gdk-pixbuf
            atk
            harfbuzz
            librsvg
            dbus
            openssl
            udev

            pkgconf
          ];

          # nativeBuildInputs = dlopenLibraries;
          LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
        };
      }
    );
}
