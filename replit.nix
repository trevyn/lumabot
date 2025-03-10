{pkgs}: {
  deps = [
    pkgs.jq
    pkgs.postgresql
    pkgs.pkg-config
    pkgs.openssl
  ];
}
