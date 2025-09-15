{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";

    systems.url = "github:nix-systems/default";
  };

  outputs = {
    self,
    systems,
    nixpkgs,
    ...
  } @ inputs: let
    eachSystem = f:
      nixpkgs.lib.genAttrs (import systems) (
        system:
          f (import nixpkgs {
            inherit system;
            overlays = [inputs.rust-overlay.overlays.default];
          })
      );

    rustToolchain = eachSystem (pkgs: (pkgs.rust-bin.stable.latest.default.override {
      extensions = ["rust-src"];
    }));
  in {
    devShells = eachSystem (pkgs: let
      dlopened = with pkgs; [libappindicator-gtk3];
    in {
      # Based on a discussion at https://github.com/oxalica/rust-overlay/issues/129
      default = pkgs.mkShell (with pkgs; {
        nativeBuildInputs =
          [
            clang

            gdb
            xdo
            # Use mold when we are runnning in Linux
            (lib.optionals stdenv.isLinux mold)
          ]
          ++ dlopened;
        buildInputs = [
          rustToolchain.${pkgs.system}
          rust-analyzer-unwrapped
          cargo
          pkg-config
          glib
          gtk3
          protobuf
          xdotool
          # openssl
        ];
        RUST_SRC_PATH = rustPlatform.rustLibSrc;
        RUSTFLAGS = "-C link-arg=-Wl,-rpath,${lib.makeLibraryPath dlopened}";
      });
    });
  };
}
