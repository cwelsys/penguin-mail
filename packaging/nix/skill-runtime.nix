{
  buildEnv,
  bash,
  coreutils,
  findutils,
  gnugrep,
  gnused,
  gawk,
  util-linux,
  python3,
  curl,
  jq,
  cacert,
}:
buildEnv {
  name = "penguin-mail-skill-runtime";
  paths = [
    bash
    coreutils
    findutils
    gnugrep
    gnused
    gawk
    util-linux
    python3
    curl
    jq
    cacert
  ];
  pathsToLink = [
    "/bin"
    "/etc/ssl/certs"
  ];
}
