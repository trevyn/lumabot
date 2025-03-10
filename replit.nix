{pkgs}: {
  deps = [
    pkgs.postgresql
    pkgs.pkg-config
    pkgs.openssl
  ];
}
