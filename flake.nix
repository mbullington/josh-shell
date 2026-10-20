{
  description = "Josh — a Unix-first JavaScript Object Shell";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          josh-shell = pkgs.callPackage ./package.nix { };
        in
        {
          inherit josh-shell;
          default = josh-shell;
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = nixpkgs.lib.getExe self.packages.${system}.default;
          meta = self.packages.${system}.default.meta;
        };
      });

      overlays.default = _final: prev: {
        josh-shell = prev.callPackage ./package.nix { };
      };
    };
}
