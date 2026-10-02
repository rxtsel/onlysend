{
  description = "Dev environment for OnlySend project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {nixpkgs, ...}: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
    };
  in {
    devShells.${system}.default = pkgs.mkShell {
      packages = with pkgs; [
        # Rust
        rustc
        cargo
        clippy
        rustfmt

        # Node
        nodejs-slim_26

        # Build tools
        pkg-config
        gcc
        gnumake
        file

        # Tauri / GTK
        glib
        gtk3
        webkitgtk_4_1
        librsvg
        openssl
        desktop-file-utils

        # WebKit's multimedia backend requires the app plugin (appsink).
        gst_all_1.gstreamer
        gst_all_1.gst-plugins-base
      ];

      GST_PLUGIN_SYSTEM_PATH_1_0 = pkgs.lib.makeSearchPath "lib/gstreamer-1.0" [
        pkgs.gst_all_1.gstreamer
        pkgs.gst_all_1.gst-plugins-base
      ];

      shellHook = ''
        export RUST_BACKTRACE=1
        export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH"
      '';
    };
  };
}
