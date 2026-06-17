{
  description = "Linux-first gamepad remapper with Qt UI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        qtEnv = pkgs.symlinkJoin {
          name = "padproxy-qt-env";
          paths = [
            pkgs.qt6.qtbase
            pkgs.qt6.qtdeclarative
          ];
        };
        qmakeForCxxQt = pkgs.writeShellScriptBin "padproxy-qmake6" ''
          real_qmake=${qtEnv}/bin/qmake6
          if [ "$1" = "-query" ]; then
            case "$2" in
              QT_HOST_LIBEXECS*|QT_INSTALL_LIBEXECS*)
                echo "${qtEnv}/libexec"
                exit 0
                ;;
              QT_HOST_BINS*|QT_INSTALL_BINS*)
                echo "${qtEnv}/bin"
                exit 0
                ;;
              QT_INSTALL_LIBS*)
                echo "${qtEnv}/lib"
                exit 0
                ;;
              QT_INSTALL_PREFIX*)
                echo "${qtEnv}"
                exit 0
                ;;
              QT_INSTALL_QML*)
                echo "${qtEnv}/lib/qt-6/qml"
                exit 0
                ;;
            esac
          fi
          exec "$real_qmake" "$@"
        '';
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "padproxy";
          version = "0.1.0";
          src = self;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [
            pkgs.makeWrapper
            pkgs.pkg-config
            qmakeForCxxQt
            pkgs.qt6.wrapQtAppsHook
          ];

          buildInputs = [
            pkgs.libevdev
            pkgs.libglvnd
            pkgs.qt6.qtbase
            pkgs.qt6.qtdeclarative
            qtEnv
          ];

          QMAKE = "${qmakeForCxxQt}/bin/padproxy-qmake6";
          QT_VERSION_MAJOR = "6";

          preBuild = ''
            export QMAKE=${qmakeForCxxQt}/bin/padproxy-qmake6
            export QT_VERSION_MAJOR=6
          '';

          preCheck = ''
            export QMAKE=${qmakeForCxxQt}/bin/padproxy-qmake6
            export QT_VERSION_MAJOR=6
          '';

          postInstall = ''
            install -Dm644 profiles/nes-2button-xa.yaml \
              $out/share/padproxy/profiles/nes-2button-xa.yaml
          '';

          postFixup = ''
            wrapProgram $out/bin/padproxyctl \
              --set-default PADPROXY_PROFILE_DIR "$out/share/padproxy/profiles"
          '';

          qtWrapperArgs = [
            "--set-default"
            "PADPROXY_PROFILE_DIR"
            "${placeholder "out"}/share/padproxy/profiles"
          ];
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/padproxy";
          meta.description = "Linux-first gamepad remapper with a Qt UI";
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.clippy
            pkgs.rustc
            pkgs.rustfmt
            pkgs.pkg-config
            pkgs.libevdev
            pkgs.libglvnd
            qmakeForCxxQt
            qtEnv
            pkgs.gh
          ];

          shellHook = ''
            export QMAKE=${qmakeForCxxQt}/bin/padproxy-qmake6
            export QT_PLUGIN_PATH=${qtEnv}/${pkgs.qt6.qtbase.qtPluginPrefix}
            export QT_VERSION_MAJOR=6
          '';
        };
      });
}
