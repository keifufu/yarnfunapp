{
  description = "yarnfunapp";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
      manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
      nativeBuildInputs = with pkgs; [pkg-config];
      buildInputs = with pkgs; [gtk4 gtk4-layer-shell];
    in {
      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = manifest.name;
        version = manifest.version;
        src = pkgs.lib.cleanSource ./.;
        cargoLock.lockFile = ./Cargo.lock;
        inherit nativeBuildInputs buildInputs;
      };
      devShells.default = pkgs.mkShell {
        packages = with pkgs; [cargo];
        shellHook = "exec ${pkgs.zsh}/bin/zsh";
        inherit nativeBuildInputs buildInputs;
      };
    });
}
