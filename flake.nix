{
  description = "Logos Inspector QML UI plugin and standalone app";

  inputs = {
    logos-module-builder.url = "github:logos-co/logos-module-builder/0.2.0";
    logos-module-builder.inputs.logos-protocol.follows = "logos-protocol";
    logos-protocol.url = "path:./nix/logos-protocol-overlay";
    logos-protocol.inputs.logos-nix.follows = "logos-module-builder/logos-nix";
    logos-protocol.inputs.nixpkgs.follows = "logos-module-builder/nixpkgs";
    blockchain_module = {
      url = "github:3esmit/logos-blockchain-module?rev=8ed201e862f15c0f1af5fd18461172e3e82173d1";
      inputs.logos-module-builder.follows = "logos-module-builder";
    };
    storage_module = {
      url = "github:3esmit/logos-storage-module?rev=cb1f934a13e35016553c670489af5fc1df8169e6";
      inputs.logos-module-builder.follows = "logos-module-builder";
    };
    delivery_module = {
      url = "github:3esmit/logos-delivery-module?rev=97788dc83b4dbeabfc73eb2367d9830a6b117a6f";
      inputs.logos-module-builder.follows = "logos-module-builder";
    };
    lez_core = {
      url = "github:3esmit/logos-execution-zone-module?rev=930262a80f7d934acd88244ba130ced786bff83b";
      inputs.logos-module-builder.follows = "logos-module-builder";
    };
    nix-bundle-dir = {
      url = "github:logos-co/nix-bundle-dir?rev=4f72d7a64dd83979d771c17161f23ebc9dbedb40";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-bundle-appimage = {
      url = "github:logos-co/nix-bundle-appimage?rev=8fcc56b5afcc313ca917cf3487be082ae2f0184c";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.nix-bundle-dir.follows = "nix-bundle-dir";
    };
    nix-bundle-macos-app = {
      url = "github:logos-co/nix-bundle-macos-app?rev=d6b0cc518e599ab7a52258bf3e1f8123c8a01d31";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.nix-bundle-dir.follows = "nix-bundle-dir";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = inputs@{
    logos-module-builder,
    nix-bundle-appimage,
    nix-bundle-dir,
    nix-bundle-macos-app,
    nixpkgs,
    ...
  }:
    let
      lib = nixpkgs.lib;
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      packageVersion = cargoToml.workspace.package.version;
      buildArtifacts = builtins.fromJSON (builtins.readFile ./build-artifacts.json);

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

      standaloneReleaseSystems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];

      coreSystems = standaloneSystems;

      circuitsRelease = buildArtifacts.circuits.release;
      circuitsTargets = buildArtifacts.circuits.targets;

      rapidsnarkVersion = buildArtifacts.rapidsnark.version;
      rapidsnarkTargets = buildArtifacts.rapidsnark.targets;
      lezRevision = buildArtifacts.lez.revision;
      lezSourceHash = buildArtifacts.lez.sourceHash;
      # Testnet v0.2 sequencers were deployed with these proof artifacts.
      # Keep their prover in a separate binary so current and historical
      # artifact trees are never linked into the same Cargo build.
      testnetV02LezRevision = "1a94c8612fa7fcc4acecde44b973d942eab19f11";
      testnetV02LezSourceHash = "sha256-GBvyfRoJc41wBE4XG1mf5vSWLbR1V17wrqdHi9RHze8=";
      testnetV02HelperBinaryName = "logos-inspector-testnet-v02-helper";
      risc0RecursionArtifactHash = "744b999f0a35b3c86753311c7efb2a0054be21727095cf105af6ee7d3f4d8849";

      standaloneShareDir = "share/logos-inspector";
      standaloneQmlEnvVar = "LOGOS_INSPECTOR_QML_DIR";
      standaloneQmlSubdir = "${standaloneShareDir}/qml";
      standaloneIconsSubdir = "${standaloneShareDir}/icons";

      rapidsnarkAssetUrl = system:
        rapidsnarkTargets.${system}.url;

      mkRapidsnark = pkgs:
        let
          system = pkgs.stdenv.hostPlatform.system;
        in
        pkgs.stdenv.mkDerivation {
          pname = "rapidsnark";
          version = rapidsnarkVersion;
          src = pkgs.fetchzip {
            url = rapidsnarkAssetUrl system;
            hash = rapidsnarkTargets.${system}.hash;
          };
          phases = [ "installPhase" ];
          installPhase = ''
            mkdir -p "$out"
            cp "$src"/lib/* "$out"/
          '';
        };

      mkLezSource = pkgs:
        pkgs.fetchzip {
          url = "https://github.com/logos-blockchain/logos-execution-zone/archive/${lezRevision}.tar.gz";
          hash = lezSourceHash;
        };

      mkTestnetV02LezSource = pkgs:
        pkgs.fetchzip {
          url = "https://github.com/logos-blockchain/logos-execution-zone/archive/${testnetV02LezRevision}.tar.gz";
          hash = testnetV02LezSourceHash;
        };

      mkRisc0RecursionArtifact = pkgs:
        pkgs.fetchurl {
          url = "https://risc0-artifacts.s3.us-west-2.amazonaws.com/zkr/${risc0RecursionArtifactHash}.zip";
          sha256 = risc0RecursionArtifactHash;
        };

      mkCircuitsArtifact = pkgs:
        let
          target = circuitsTargets.${pkgs.stdenv.hostPlatform.system};
        in
        pkgs.fetchzip {
          url = "https://github.com/logos-blockchain/logos-blockchain-circuits/releases/download/${circuitsRelease}/logos-blockchain-circuits-${circuitsRelease}-${target.os}-${target.arch}.tar.gz";
          hash = target.hash;
        };

      mkCircuitBuildContext = pkgs: { includeLogosBlockchainCircuits ? false }:
        let
          circuitsArtifact = mkCircuitsArtifact pkgs;
          rapidsnark = mkRapidsnark pkgs;
          lezSource = mkLezSource pkgs;
        in
        {
          inherit circuitsArtifact rapidsnark lezSource;
          env = {
            LBC_ROOT_DIR = "${circuitsArtifact}";
            RAPIDSNARK_LIB_DIR = "${rapidsnark}";
            RISC0_SKIP_BUILD = "1";
          } // lib.optionalAttrs includeLogosBlockchainCircuits {
            LOGOS_BLOCKCHAIN_CIRCUITS = "${circuitsArtifact}";
          };
        };

      linkLezArtifacts = lezSource: ''
        rm -rf /build/cargo-vendor-dir/artifacts
        ln -s ${lezSource}/artifacts /build/cargo-vendor-dir/artifacts
      '';

      qtUnifiedQmakeSetup = pkgs: ''
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

      forSystems = systems: f:
        lib.genAttrs systems
          (system: f (import nixpkgs { inherit system; }));

      forAllSystems = forSystems qmlSystems;

      # Within flake evaluation, ./. is the Git-filtered source, not the raw worktree.
      sourceSets = import ./nix/source-sets.nix {
        inherit lib;
        root = ./.;
      };
      source = sourceSets.workspace;
      standaloneRustSource = sourceSets.standaloneRust;
      standaloneAssetSource = sourceSets.standaloneAssets;
      coreModuleSource = sourceSets.coreModule;

      mkTestnetV02HelperBinary = pkgs:
        let
          circuitBuild = mkCircuitBuildContext pkgs { };
          testnetV02LezSource = mkTestnetV02LezSource pkgs;
          risc0RecursionArtifact = mkRisc0RecursionArtifact pkgs;
          # RISC Zero invokes `xcrun metal` and `xcrun metallib` on Darwin.
          # Nix's SDK variables hide the host Metal toolchain, so clear them
          # only for those calls while preserving the regular Nix environment.
          metalXcrun = pkgs.writeShellScriptBin "xcrun" ''
            tool=
            for argument in "$@"; do
              case "$argument" in metal|metallib) tool=1 ;; esac
            done
            if [ -n "$tool" ]; then
              unset DEVELOPER_DIR SDKROOT
            fi
            exec /usr/bin/xcrun "$@"
          '';
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = testnetV02HelperBinaryName;
          version = packageVersion;
          src = standaloneRustSource;
          cargoRoot = "crates/testnet-v02-helper";
          # buildRustPackage installs from the source-root target directory.
          # Keep this nested workspace's build output there instead of passing
          # a manifest path that makes Cargo write under the helper directory.
          buildAndTestSubdir = "crates/testnet-v02-helper";
          cargoLock = {
            lockFile = ./crates/testnet-v02-helper/Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          env = circuitBuild.env // {
            RECURSION_SRC_PATH = "${risc0RecursionArtifact}";
          };
          nativeBuildInputs = [ pkgs.python3 ];
          preBuild = ''
            export HOME=$(mktemp -d)
            ${linkLezArtifacts testnetV02LezSource}
          '' + lib.optionalString pkgs.stdenv.isDarwin ''
            export PATH="${metalXcrun}/bin:$PATH:/usr/bin"
          '';
          doCheck = false;
          meta.mainProgram = testnetV02HelperBinaryName;
        };

      qmlModule = logos-module-builder.lib.mkLogosQmlModule {
        src = source;
        configFile = ./metadata.json;
        flakeInputs = inputs;
      };

      mkCoreFfiPackage = pkgs:
        let
          circuitBuild = mkCircuitBuildContext pkgs {
            includeLogosBlockchainCircuits = true;
          };
          testnetV02Helper = mkTestnetV02HelperBinary pkgs;
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "logos-inspector-core-ffi";
          version = packageVersion;
          src = standaloneRustSource;
          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          cargoBuildFlags = [
            "--package"
            "logos-inspector-core-ffi"
          ];
          env = circuitBuild.env // {
            LOGOS_INSPECTOR_TESTNET_V02_HELPER = "${testnetV02Helper}/bin/${testnetV02HelperBinaryName}";
          };
          nativeBuildInputs = [ pkgs.python3 ];
          preBuild = linkLezArtifacts circuitBuild.lezSource;
          postInstall = ''
            mkdir -p "$out/include"
            cp ${./crates/core-ffi/include/logos_inspector_core.h} "$out/include/logos_inspector_core.h"
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
            input = {
              packages = coreFfiPackages;
            };
            packages.default = "default";
          };
        };
        tests = {
          dir = coreModuleSource + "/tests";
          extraCmakeFlags = [
            "-DLOGOS_INSPECTOR_ENABLE_COMPOSED_HOST_TESTS=ON"
          ];
        };
      };

      mkStandaloneBinary = pkgs: { buildType, staticRapidsnarkFeature }:
        let
          circuitBuild = mkCircuitBuildContext pkgs { };
          testnetV02Helper = mkTestnetV02HelperBinary pkgs;
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
          version = packageVersion;
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
          env = circuitBuild.env // {
            QT_VERSION_MAJOR = "6";
          };
          doCheck = false;
          preBuild = ''
            ${linkLezArtifacts circuitBuild.lezSource}
            ${qtUnifiedQmakeSetup pkgs}
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

      mkStandalonePackage = pkgs: binary: testnetV02Helper:
        pkgs.stdenvNoCC.mkDerivation {
          pname = "logos-inspector-standalone-gui";
          version = packageVersion;
          dontUnpack = true;
          nativeBuildInputs = [ pkgs.makeWrapper ];
          installPhase = ''
            runHook preInstall

            mkdir -p "$out/bin" "$out/${standaloneShareDir}"
            cp -r ${standaloneAssetSource}/qml "$out/${standaloneQmlSubdir}"
            cp -r ${standaloneAssetSource}/icons "$out/${standaloneIconsSubdir}"
            mkdir -p "$out/libexec"
            cp ${testnetV02Helper}/bin/${testnetV02HelperBinaryName} \
              "$out/libexec/${testnetV02HelperBinaryName}"

            makeWrapper ${binary}/bin/logos-inspector-standalone-gui \
              "$out/bin/logos-inspector-standalone-gui" \
              --set ${standaloneQmlEnvVar} "$out/${standaloneQmlSubdir}" \
              --set LOGOS_INSPECTOR_TESTNET_V02_HELPER \
                "$out/libexec/${testnetV02HelperBinaryName}"

            runHook postInstall
          '';
          passthru = {
            extraDirs = [ "libexec" "share" ];
            extraClosurePaths = [ binary testnetV02Helper ];
          };
          meta.mainProgram = "logos-inspector-standalone-gui";
        };

      mkStandalonePortablePackage = pkgs: binary: testnetV02Helper:
        pkgs.stdenvNoCC.mkDerivation {
          pname = "logos-inspector-standalone-gui-portable-source";
          version = packageVersion;
          dontUnpack = true;
          installPhase = ''
            runHook preInstall

            unwrapped="${binary}/bin/.logos-inspector-standalone-gui-wrapped"
            if [ ! -x "$unwrapped" ]; then
              echo "Qt build did not expose the unwrapped standalone executable" >&2
              exit 1
            fi

            mkdir -p \
              "$out/bin" \
              "$out/libexec" \
              "$out/${standaloneShareDir}"
            cp "$unwrapped" "$out/bin/logos-inspector-standalone-gui"
            cp ${testnetV02Helper}/bin/${testnetV02HelperBinaryName} \
              "$out/libexec/${testnetV02HelperBinaryName}"
            cp -r ${standaloneAssetSource}/qml "$out/${standaloneQmlSubdir}"
            cp -r ${standaloneAssetSource}/icons "$out/${standaloneIconsSubdir}"

            runHook postInstall
          '';
          passthru = {
            extraDirs = [ "libexec" "share" ];
            extraClosurePaths = [ binary testnetV02Helper ];
          };
          meta.mainProgram = "logos-inspector-standalone-gui";
        };

      standalonePackages = forSystems standaloneSystems (pkgs:
        mkStandalonePackage
          pkgs
          (mkStandaloneBinary pkgs {
            buildType = "release";
            staticRapidsnarkFeature = true;
          })
          (mkTestnetV02HelperBinary pkgs));

      standaloneDevPackages = forSystems standaloneSystems (pkgs:
        mkStandalonePackage
          pkgs
          (mkStandaloneBinary pkgs {
            buildType = "debug";
            staticRapidsnarkFeature = false;
          })
          (mkTestnetV02HelperBinary pkgs));

      standalonePortablePackages = forSystems standaloneReleaseSystems (pkgs:
        mkStandalonePortablePackage
          pkgs
          (mkStandaloneBinary pkgs {
            buildType = "release";
            staticRapidsnarkFeature = true;
          })
          (mkTestnetV02HelperBinary pkgs));

      standaloneBundles = forSystems standaloneReleaseSystems (pkgs:
        let
          system = pkgs.stdenv.hostPlatform.system;
          standalone = standalonePortablePackages.${system};
        in
        # Qt and GLib contain inert build-prefix strings in compiled vendor
        # binaries. qtApp preserves them as visible warnings while retaining
        # strict dynamic-loader, shebang, and symlink portability checks.
        nix-bundle-dir.bundlers.${system}.qtApp standalone);

      standaloneAppImages = forSystems [ "x86_64-linux" ] (pkgs:
        let
          system = pkgs.stdenv.hostPlatform.system;
        in
        nix-bundle-appimage.lib.${system}.mkAppImage {
          drv = standalonePackages.${system};
          name = "logos-inspector-standalone";
          bundle = standaloneBundles.${system};
          desktopFile = ./packaging/logos-inspector.desktop;
          icon = ./icons/inspector.svg;
        });

      standaloneMacApps = forSystems [ "aarch64-darwin" ] (pkgs:
        let
          system = pkgs.stdenv.hostPlatform.system;
        in
        nix-bundle-macos-app.lib.${system}.mkMacOSApp {
          drv = standalonePackages.${system};
          name = "LogosInspector";
          bundle = standaloneBundles.${system};
          icon = ./icons/inspector.svg;
          infoPlist = ./packaging/Info.plist.in;
          version = packageVersion;
        });

      testnetV02HelperPackages = forSystems standaloneSystems mkTestnetV02HelperBinary;

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
      checks = lib.recursiveUpdate
        qmlModule.checks
        (builtins.mapAttrs
          (_system: coreChecks: {
            core-host-transport-integration = coreChecks.unit-tests;
          })
          (lib.getAttrs coreSystems coreModule.checks));
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
          } // lib.optionalAttrs (builtins.hasAttr system testnetV02HelperPackages) {
            "testnet-v02-helper" = testnetV02HelperPackages.${system};
          } // lib.optionalAttrs (builtins.hasAttr system standaloneBundles) {
            standalone-bundle-dir = standaloneBundles.${system};
          } // lib.optionalAttrs (builtins.hasAttr system standaloneAppImages) {
            standalone-appimage = standaloneAppImages.${system};
          } // lib.optionalAttrs (builtins.hasAttr system standaloneMacApps) {
            standalone-macos-app = standaloneMacApps.${system};
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
