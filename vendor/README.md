# Vendored patches

## `wasm-bindgen-cli-w3c-0.2.121.patch`

A small patch against the published `wasm-bindgen-cli` 0.2.121 source. It is
required because the upstream `wasm-bindgen-test-runner` 0.2.121:

1. Sends the legacy JSON-Wire `desiredCapabilities` body to ChromeDriver,
   which `chromedriver >= 75` rejects (strict W3C). The patch sends the
   W3C `capabilities.alwaysMatch` body plus an explicit `browserName: chrome`.
2. Uses a `ureq` POST that ChromeDriver/geckodriver's embedded HTTP server
   resets under WSL. The patch replaces `Client::doit` with a hand-rolled
   HTTP/1.1 over `TcpStream` (Content-Length, no `Expect: 100-continue`,
   parses headers then reads exactly `Content-Length` bytes).
3. Spawns ChromeDriver and immediately POSTs `/session` before its HTTP loop
   has finished initialising. The patch sleeps 750ms after the bind probe.

All three issues are upstream limitations; this patch is the minimum needed
to get green wasm-bindgen browser tests on WSL Ubuntu 24.04. Apply it via
`scripts/install-wasm-test-runner.sh`, which fetches the pristine crate
source from crates.io, applies the patch, and `cargo install`s the binaries
into `~/.cargo/bin`.
