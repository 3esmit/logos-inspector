{
  description = "Logos Inspector QML UI plugin and standalone app";

  inputs = {
    logos-module-builder.url = "github:logos-co/logos-module-builder/0.2.0";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = inputs@{ logos-module-builder, nixpkgs, ... }:
    let
      lib = nixpkgs.lib;

      qmlSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      standaloneSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      coreSystems = standaloneSystems;

      circuitsVersion = "0.5.3";

      circuitsTargets = {
        x86_64-linux = {
          os = "linux";
          arch = "x86_64";
          hash = "sha256-2/Fw6ko+MCO2Paiw4zUQ1eacjvtPGP2n53RL39Ibntc=";
        };
        aarch64-linux = {
          os = "linux";
          arch = "aarch64";
          hash = "sha256-5779U71gejCEOmWToziHLyh3n/DVTyEemsbMG+HbJJM=";
        };
        aarch64-darwin = {
          os = "macos";
          arch = "aarch64";
          hash = "sha256-WFKsOEx93MIT/+l8gVQpTMoAwMjMTIHD1FzL/+AFcXA=";
        };
      };

      rapidsnarkVersion = "0.0.8";

      rapidsnarkHashes = {
        x86_64-linux = "sha256-88+TkECQYCKBN0WbYLRB+qi6TEhbjVfrpCqlSgm0DR8=";
        aarch64-linux = "sha256-kcSITwWZjEv9gadzYL7LiG+DVtEEFbQJuhd/LTyOxJU=";
        aarch64-darwin = "sha256-DmhMmMq7/HKsx+Iz4rdsKYBDEwqwMZALo+ZJePJvAJg=";
        x86_64-darwin = "sha256-/GCXzzT5mkBeXkVQAGEF9OmJXXcYz4KoXNzjFvhSgNU=";
      };

      rapidsnarkAssetUrl = system:
        let
          forkBase = "https://github.com/logos-blockchain/logos-blockchain-rust-rapidsnark/releases/download/rapidsnark-pic-v${rapidsnarkVersion}";
          iden3Base = "https://github.com/iden3/rapidsnark/releases/download/v${rapidsnarkVersion}";
        in
        {
          x86_64-linux = "${forkBase}/rapidsnark-linux-x86_64-pic-v${rapidsnarkVersion}.zip";
          aarch64-linux = "${forkBase}/rapidsnark-linux-aarch64-pic-v${rapidsnarkVersion}.zip";
          aarch64-darwin = "${iden3Base}/rapidsnark-macOS-arm64-v${rapidsnarkVersion}.zip";
          x86_64-darwin = "${iden3Base}/rapidsnark-macOS-x86_64-v${rapidsnarkVersion}.zip";
        }.${system};

      mkRapidsnark = pkgs:
        let
          system = pkgs.stdenv.hostPlatform.system;
        in
        pkgs.stdenv.mkDerivation {
          pname = "rapidsnark";
          version = rapidsnarkVersion;
          src = pkgs.fetchzip {
            url = rapidsnarkAssetUrl system;
            hash = rapidsnarkHashes.${system};
          };
          phases = [ "installPhase" ];
          installPhase = ''
            mkdir -p "$out"
            cp "$src"/lib/* "$out"/
          '';
        };

      mkCircuitsArtifact = pkgs:
        let
          target = circuitsTargets.${pkgs.stdenv.hostPlatform.system};
        in
        pkgs.fetchzip {
          url = "https://github.com/logos-blockchain/logos-blockchain-circuits/releases/download/v${circuitsVersion}/logos-blockchain-circuits-v${circuitsVersion}-${target.os}-${target.arch}.tar.gz";
          hash = target.hash;
        };

      forSystems = systems: f:
        lib.genAttrs systems
          (system: f (import nixpkgs { inherit system; }));

      forAllSystems = forSystems qmlSystems;

      workspaceRoot = toString ./.;

      relativeToRoot = path:
        let
          pathString = toString path;
          prefix = workspaceRoot + "/";
        in
        if pathString == workspaceRoot then "" else lib.removePrefix prefix pathString;

      sourceFilter = path: type:
        let
          name = builtins.baseNameOf path;
          ignoredDirectories = [
            ".direnv"
            "coverage"
            "dist"
            "node_modules"
            "target"
            "tmp"
          ];
          isIgnoredDirectory = type == "directory" && builtins.elem name ignoredDirectories;
          isHiddenDirectory = type == "directory" && lib.hasPrefix "." name;
          isResultLink = name == "result" || lib.hasPrefix "result-" name;
          isLogFile = lib.hasSuffix ".log" name;
        in
        lib.cleanSourceFilter path type
        && !isIgnoredDirectory
        && !isHiddenDirectory
        && !isResultLink
        && !isLogFile;

      source = lib.cleanSourceWith {
        src = ./.;
        filter = sourceFilter;
      };

      standaloneRustSource = lib.cleanSourceWith {
        src = ./.;
        filter = path: type:
          let
            rel = relativeToRoot path;
            isRuntimeAsset =
              rel == "qml" || lib.hasPrefix "qml/" rel
              || rel == "icons" || lib.hasPrefix "icons/" rel;
          in
          sourceFilter path type && !isRuntimeAsset;
      };

      standaloneAssetSource = lib.cleanSourceWith {
        src = ./.;
        filter = path: type:
          let
            rel = relativeToRoot path;
            isRuntimeAsset =
              rel == ""
              || rel == "qml" || lib.hasPrefix "qml/" rel
              || rel == "icons" || lib.hasPrefix "icons/" rel;
          in
          sourceFilter path type && isRuntimeAsset;
      };

      coreModuleSource = lib.cleanSourceWith {
        src = ./core;
        filter = sourceFilter;
      };

      qmlModule = logos-module-builder.lib.mkLogosQmlModule {
        src = source;
        configFile = ./metadata.json;
        flakeInputs = inputs;
      };

      mkCoreFfiPackage = pkgs:
        let
          circuitsArtifact = mkCircuitsArtifact pkgs;
          lezSource = pkgs.fetchzip {
            url = "https://github.com/logos-blockchain/logos-execution-zone/archive/e37876a64028a335eb693198a1ed6a0e875ec5b4.tar.gz";
            hash = "sha256-ltLcysXUdVUXAe25Tl8x7e7ZsTzj1sHlyS3glp97TAo=";
          };
          rapidsnark = mkRapidsnark pkgs;
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "logos-inspector-core-ffi";
          version = "0.2.0-rc6";
          src = standaloneRustSource;
          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          cargoBuildFlags = [
            "--package"
            "logos-inspector-core-ffi"
          ];
          env = {
            LBC_ROOT_DIR = "${circuitsArtifact}";
            LOGOS_BLOCKCHAIN_CIRCUITS = "${circuitsArtifact}";
            RAPIDSNARK_LIB_DIR = "${rapidsnark}";
            RISC0_SKIP_BUILD = "1";
          };
          nativeBuildInputs = [ pkgs.python3 ];
          preBuild = ''
            rm -rf /build/cargo-vendor-dir/artifacts
            ln -s ${lezSource}/artifacts /build/cargo-vendor-dir/artifacts
          '';
          postInstall = ''
            mkdir -p "$out/include"
            cp ${./core/lib/logos_inspector_core.h} "$out/include/"
          '';
          doCheck = false;
        };

      coreFfiPackages = forSystems coreSystems (pkgs: {
        default = mkCoreFfiPackage pkgs;
      });

      coreModule = logos-module-builder.lib.mkLogosModule {
        src = coreModuleSource;
        configFile = ./core/metadata.json;
        flakeInputs = inputs;
        externalLibInputs = {
          logos_inspector_core = {
            packages = coreFfiPackages;
          };
        };
      };

      mkStandaloneBinary = pkgs: { buildType, staticRapidsnarkFeature }:
        let
          circuitsArtifact = mkCircuitsArtifact pkgs;
          lezSource = pkgs.fetchzip {
            url = "https://github.com/logos-blockchain/logos-execution-zone/archive/e37876a64028a335eb693198a1ed6a0e875ec5b4.tar.gz";
            hash = "sha256-ltLcysXUdVUXAe25Tl8x7e7ZsTzj1sHlyS3glp97TAo=";
          };
          rapidsnark = mkRapidsnark pkgs;
          qtInputs = with pkgs.qt6; [
            qtbase
            qtdeclarative
            qttools
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            qtwayland
          ];
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "logos-inspector-standalone-gui-bin";
          version = "0.2.0-rc6";
          src = standaloneRustSource;
          inherit buildType;
          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          cargoBuildFlags = [
            "--package"
            "logos-inspector-standalone-gui"
          ] ++ lib.optionals staticRapidsnarkFeature [
            "--features" "static-rapidsnark-link"
          ];
          cargoTestFlags = [ "--package" "logos-inspector-standalone-gui" ];
          nativeBuildInputs = [
            pkgs.python3
            pkgs.cmake
            pkgs.pkg-config
            pkgs.qt6.qmake
            pkgs.qt6.wrapQtAppsHook
          ];
          buildInputs = qtInputs;
          env = {
            LBC_ROOT_DIR = "${circuitsArtifact}";
            RAPIDSNARK_LIB_DIR = "${rapidsnark}";
            QT_VERSION_MAJOR = "6";
            RISC0_SKIP_BUILD = "1";
          };
          doCheck = false;
          preBuild = ''
            rm -rf /build/cargo-vendor-dir/artifacts
            ln -s ${lezSource}/artifacts /build/cargo-vendor-dir/artifacts

            qtBuildRoot="$TMPDIR/qt-unified"
            mkdir -p "$qtBuildRoot/bin" "$qtBuildRoot/include" "$qtBuildRoot/lib" "$qtBuildRoot/libexec" "$qtBuildRoot/plugins"

            for qtDir in ${pkgs.qt6.qtbase} ${pkgs.qt6.qtdeclarative}; do
              if [ -d "$qtDir/bin" ]; then
                for item in "$qtDir"/bin/*; do
                  ln -sfn "$item" "$qtBuildRoot/bin/$(basename "$item")"
                done
              fi
              if [ -d "$qtDir/include" ]; then
                for item in "$qtDir"/include/*; do
                  ln -sfn "$item" "$qtBuildRoot/include/$(basename "$item")"
                done
              fi
              if [ -d "$qtDir/lib" ]; then
                for item in "$qtDir"/lib/*; do
                  ln -sfn "$item" "$qtBuildRoot/lib/$(basename "$item")"
                done
              fi
              if [ -d "$qtDir/libexec" ]; then
                for item in "$qtDir"/libexec/*; do
                  ln -sfn "$item" "$qtBuildRoot/libexec/$(basename "$item")"
                done
              fi
              if [ -d "$qtDir/lib/qt-6/plugins" ]; then
                for item in "$qtDir"/lib/qt-6/plugins/*; do
                  ln -sfn "$item" "$qtBuildRoot/plugins/$(basename "$item")"
                done
              fi
            done

            rm -f "$qtBuildRoot/bin/qmake" "$qtBuildRoot/bin/qmake6"
            cat > "$qtBuildRoot/bin/qmake6" <<EOF
#!/bin/sh
if [ "\$1" = "-query" ]; then
  case "\$2" in
    QT_HOST_BINS | QT_HOST_BINS/get | QT_INSTALL_BINS | QT_INSTALL_BINS/get) echo "$qtBuildRoot/bin"; exit 0 ;;
    QT_HOST_LIBEXECS | QT_HOST_LIBEXECS/get | QT_INSTALL_LIBEXECS | QT_INSTALL_LIBEXECS/get) echo "$qtBuildRoot/libexec"; exit 0 ;;
    QT_INSTALL_HEADERS) echo "$qtBuildRoot/include"; exit 0 ;;
    QT_INSTALL_LIBS) echo "$qtBuildRoot/lib"; exit 0 ;;
    QT_INSTALL_PLUGINS) echo "$qtBuildRoot/plugins"; exit 0 ;;
    QT_INSTALL_PREFIX) echo "$qtBuildRoot"; exit 0 ;;
  esac
fi
exec ${pkgs.qt6.qtbase}/bin/qmake6 "\$@"
EOF
            chmod +x "$qtBuildRoot/bin/qmake6"
            export QMAKE="$qtBuildRoot/bin/qmake6"
          '';
          preFixup = ''
            ${lib.optionalString pkgs.stdenv.isLinux ''
              if [ -x "$out/bin/logos-inspector-standalone-gui" ]; then
                ${pkgs.patchelf}/bin/patchelf \
                  --set-rpath "${lib.makeLibraryPath (qtInputs ++ [ pkgs.stdenv.cc.cc.lib pkgs.python3 ])}" \
                  "$out/bin/logos-inspector-standalone-gui"
              fi
            ''}
            qtWrapperArgs+=(
              --set-default QT_QUICK_BACKEND software
              --set-default QSG_RHI_BACKEND software
            )
          '';
          meta.mainProgram = "logos-inspector-standalone-gui";
        };

      mkStandalonePackage = pkgs: binary:
        pkgs.stdenvNoCC.mkDerivation {
          pname = "logos-inspector-standalone-gui";
          version = "0.2.0-rc6";
          dontUnpack = true;
          nativeBuildInputs = [ pkgs.makeWrapper ];
          installPhase = ''
            runHook preInstall

            mkdir -p "$out/bin" "$out/share/logos-inspector"
            cp -r ${standaloneAssetSource}/qml "$out/share/logos-inspector/qml"
            cp -r ${standaloneAssetSource}/icons "$out/share/logos-inspector/icons"

            makeWrapper ${binary}/bin/logos-inspector-standalone-gui \
              "$out/bin/logos-inspector-standalone-gui" \
              --set LOGOS_INSPECTOR_QML_DIR "$out/share/logos-inspector/qml"

            runHook postInstall
          '';
          meta.mainProgram = "logos-inspector-standalone-gui";
        };

      standalonePackages = forSystems standaloneSystems (pkgs:
        mkStandalonePackage pkgs (mkStandaloneBinary pkgs {
          buildType = "release";
          staticRapidsnarkFeature = true;
        }));

      standaloneDevPackages = forSystems standaloneSystems (pkgs:
        mkStandalonePackage pkgs (mkStandaloneBinary pkgs {
          buildType = "debug";
          staticRapidsnarkFeature = false;
        }));

      mkRenamedLgxPackage = pkgs: { source, outputName }:
        pkgs.runCommand (lib.removeSuffix ".lgx" outputName) { } ''
          shopt -s nullglob
          files=("${source}"/*.lgx)
          if [ "''${#files[@]}" -ne 1 ]; then
            echo "expected exactly one .lgx in ${source}, got ''${#files[@]}" >&2
            exit 1
          fi

          mkdir -p "$out"
          cp "''${files[0]}" "$out/${outputName}"
        '';
    in
    qmlModule // {
      packages = builtins.mapAttrs
        (system: packages:
          let
            pkgs = import nixpkgs { inherit system; };
            uiLgx = mkRenamedLgxPackage pkgs {
              source = packages.lgx-portable;
              outputName = "logos-inspector-ui-module.lgx";
            };
            coreLgx = mkRenamedLgxPackage pkgs {
              source = coreModule.packages.${system}.lgx-portable;
              outputName = "logos-inspector-lib.lgx";
            };
          in
          packages // {
            lgx = uiLgx;
            lgx-portable = uiLgx;
          } // lib.optionalAttrs (builtins.hasAttr system standalonePackages) {
            standalone = standalonePackages.${system};
          } // lib.optionalAttrs (builtins.hasAttr system standaloneDevPackages) {
            standalone-dev = standaloneDevPackages.${system};
          } // lib.optionalAttrs (builtins.elem system coreSystems) {
            core = coreModule.packages.${system}.default;
            core-ffi = coreFfiPackages.${system}.default;
            core-lib = coreModule.packages.${system}.lib;
            core-lgx = coreLgx;
            core-lgx-portable = coreLgx;
            logos_inspector = coreModule.packages.${system}.default;
            logos_inspector-lgx = coreLgx;
          })
        qmlModule.packages;
      apps = builtins.mapAttrs
        (system: apps:
          apps // { qml-ui = apps.default; }
          // lib.optionalAttrs (builtins.hasAttr system standalonePackages) {
            standalone = {
              type = "app";
              program = "${standalonePackages.${system}}/bin/logos-inspector-standalone-gui";
            };
          } // lib.optionalAttrs (builtins.hasAttr system standaloneDevPackages) {
            standalone-dev = {
              type = "app";
              program = "${standaloneDevPackages.${system}}/bin/logos-inspector-standalone-gui";
            };
          })
        qmlModule.apps;
    };
}
