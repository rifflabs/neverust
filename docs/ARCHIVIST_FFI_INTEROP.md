# Archivist FFI Interop

This repository now includes an embedded FFI path to the real Nim Archivist node (`../archivist-node`) without using a subprocess control path from Rust.

## What was added

- Nim FFI shim: `neverust-core/ffi/archivist_ffi.nim`
  - `archivist_ffi_start(...)`
  - `archivist_ffi_poll()`
  - `archivist_ffi_stop(...)`
  - `archivist_ffi_last_error(...)`
- Rust interop proof test: `neverust-core/tests/archivist_ffi_interop.rs`
  - Compiles shim to `target/archivist-ffi/libarchivist_ffi.so`
  - Starts embedded Archivist node via FFI
  - Uploads content to Archivist
  - Retrieves same CID via Neverust `/network/stream`
- Neverust fallback peer override:
  - `NEVERUST_HTTP_FALLBACK_PEERS=http://host:port[,http://host:port...]`

## Run the proof test

```bash
cargo test -p neverust-core ffi_embedded_archivist_upload_then_neverust_retrieve -- --nocapture
```

To compile against a specific Archivist checkout:

```bash
export ARCHIVIST_NODE_FFI_ROOT=/absolute/path/to/archivist-node
cargo test -p neverust-core ffi_embedded_archivist_upload_then_neverust_retrieve -- --nocapture
```

## Port forwarding matrix

Forward these for full cross-host interoperability:

### Neverust node

- `TCP 8070` (`--listen-port`): libp2p transport
- `UDP 8090` (`--disc-port`): discovery (DiscV5)
- `UDP 8091` (`disc-port + 1`): BoTG/UDP
- `TCP 8080` (`--api-port`): Archivist-compatible HTTP API

### Archivist node

- `TCP <listen-addrs tcp port>` (default random if `/tcp/0`): libp2p transport
- `UDP 8090` (`--disc-port` default): discovery
- `TCP 8080` (`--api-port` default): Archivist HTTP API
- `TCP 8008` (`--metrics-port`, optional): metrics endpoint

## IP targets to forward to

Use the private IP of each node host in your forwarding rules:

- Neverust host: `<neverust_host_private_ip>`
- Archivist host: `<archivist_host_private_ip>`

And if you are forcing HTTP fallback from Neverust, set:

```bash
export NEVERUST_HTTP_FALLBACK_PEERS=http://<archivist_host_private_ip>:8080
```
