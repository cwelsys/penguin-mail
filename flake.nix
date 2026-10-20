{
  description = "Penguin Mail, mail and calendar for Linux";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forSystems = nixpkgs.lib.genAttrs systems;
      packagesFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          penguin-mail = pkgs.callPackage ./packaging/nix/package.nix { };
        in
        {
          inherit pkgs penguin-mail;
        };
    in
    {
      packages = forSystems (
        system:
        let
          package = (packagesFor system).penguin-mail;
        in
        {
          default = package;
          penguin-mail = package;
        }
      );

      apps = forSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/penguin-mail";
        };
        cli = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/penguin-mail-cli";
        };
      });

      checks = forSystems (
        system:
        let
          inherit (packagesFor system) pkgs;
          package = self.packages.${system}.default;
        in
        {
          inherit package;
          cli = pkgs.testers.testVersion {
            inherit package;
            command = ''env HOME="$TMPDIR" penguin-mail-cli --version'';
          };
          app = pkgs.testers.testVersion {
            inherit package;
            command = ''env HOME="$TMPDIR" penguin-mail --version'';
          };
        }
      );

      formatter = forSystems (system: (packagesFor system).pkgs.nixfmt-tree);

      devShells = forSystems (
        system:
        let
          inherit (packagesFor system) pkgs penguin-mail;
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ penguin-mail ];
            PENGUIN_MAIL_SKILL_RUNTIME = "${penguin-mail.skillRuntime}";
            packages = with pkgs; [
              cargo
              rustc
              clippy
              rustfmt
              rust-analyzer
              gnupg
              bubblewrap
              dbus
              xvfb-run
              python3
            ];
            shellHook = ''
              export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH:${
                pkgs.lib.makeSearchPath "share" [
                  pkgs.adwaita-icon-theme
                  pkgs.shared-mime-info
                ]
              }''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
              export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
            '';
          };
        }
      );
    };
}
