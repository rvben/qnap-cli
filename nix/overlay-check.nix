{
  pkgs,
  overlay,
  packageName,
}:

let
  consumer = pkgs.extend (
    _final: prev: {
      rustPlatform = prev.rustPlatform // {
        buildRustPackage =
          args:
          (prev.rustPlatform.buildRustPackage args)
          // {
            usesConsumerRustPlatform = true;
          };
      };
    }
  );
  package = (consumer.extend overlay).${packageName};
in
assert package.usesConsumerRustPlatform or false;
pkgs.runCommand "${packageName}-overlay-check" { } ''
  touch "$out"
''
