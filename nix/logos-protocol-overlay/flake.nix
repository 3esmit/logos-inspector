{
  description = "Patched Logos Protocol for reliable asynchronous C ABI calls";

  inputs = {
    logos-nix.url = "github:logos-co/logos-nix";
    nixpkgs.follows = "logos-nix/nixpkgs";

    upstream.url = "github:logos-co/logos-protocol/d7ad26d369c4e464a99f2a357f10c5947c7174e1";
    upstream.inputs.logos-nix.follows = "logos-nix";
    upstream.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, upstream, ... }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system: f {
          inherit system;
          pkgs = import nixpkgs { inherit system; };
        });
      protocolPatch = ./patches/0001-preserve-async-outcomes-and-lifecycle.patch;
      patchedAttrs = old: {
        version = "0.2.1";
        __intentionallyOverridingVersion = true;
        patches = (old.patches or [ ]) ++ [ protocolPatch ];
      };
      facadeHeaders = builtins.attrNames (builtins.readDir ./cpp);
    in
    {
      packages = forAllSystems ({ pkgs, system }:
        let
          upstreamPackages = upstream.packages.${system};
          protocolLib = upstreamPackages.logos-protocol-lib.overrideAttrs patchedAttrs;
          protocolInclude = upstreamPackages.logos-protocol-include.overrideAttrs patchedAttrs;
          tests = upstreamPackages.tests.overrideAttrs patchedAttrs;
          protocol = pkgs.symlinkJoin {
            name = "logos-protocol-0.2.1";
            paths = [ protocolLib protocolInclude ];
            propagatedBuildInputs = protocolLib.propagatedBuildInputs or [ ];
          };
        in
        {
          logos-protocol-lib = protocolLib;
          logos-protocol-include = protocolInclude;
          logos-protocol = protocol;
          inherit tests;
          default = protocol;
        });

      checks = forAllSystems ({ pkgs, system }:
        let
          packages = self.packages.${system};
          sourceFacade = pkgs.runCommand "logos-protocol-source-facade-0.2.1" { } ''
            for header in ${builtins.concatStringsSep " " facadeHeaders}; do
              diff -q -Z "${./cpp}/$header" \
                  "${packages.logos-protocol-include}/include/cpp/$header"
            done
            touch "$out"
          '';
        in
        {
          inherit sourceFacade;
          tests = packages.tests;
        });
    };
}
