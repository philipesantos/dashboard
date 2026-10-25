# Contract: CLI changes

## `runtime pack` on a core module

```text
runtime pack <module-or-component> --manifest <file> --output <file>
```

- Input may now be a core WebAssembly module, such as cargo's `wasm32-unknown-unknown` artifact,
  or a component, as before. A module is turned into a component with `wit-component`
  `=0.262.0` (validation on), from the component type wit-bindgen embeds in it, and then packed.
  For every example, the result equals what `cargo xtask plugins` builds, byte for byte.
- Output, unchanged: `packed <output>` and `digest: <sha-256>`; every path escaped.
- Refusals, as one line each:
  - `error: invalid-package: <file> is a core module without the contract's world (build it with wit-bindgen)`
    when the module embeds no component type;
  - `error: invalid-package: <cause>` when the encoder refuses the module, with the encoder's
    cause escaped on one line (it quotes names taken from the module; a test feeds it newlines
    and control characters);
  - every refusal of today (`manifest-mismatch` for a component that already has a manifest,
    `invalid-manifest`, the size cap) unchanged, applied to the resulting component.
- The input's size cap stays 10 MiB; the component made from it is checked against the same cap.

## `runtime-test-network` (test builds only)

- Built only with the `test-network` feature (`required-features`), which refuses release
  builds; never installed, never named in the docs.
- Same commands, options and output as `runtime`; its runtime is
  `Runtime::with_test_network_on_keychain`, with routes, a test CA and a data directory read from
  the JSON file named by `DASHBOARD_TEST_NETWORK`, and the development vault on the system's
  keychain (each command is a process of its own). The file's data directory replaces the
  default development one; `--data` still overrides it.
- `accounts connect` does not open a browser: it fetches the sign-in address and follows the
  test sign-in service's approval itself, then waits for the callback as `runtime` does. The
  client that follows it lives in `runtime/src/network/`, behind `test-network`.
- Missing or unreadable `DASHBOARD_TEST_NETWORK`: it exits with a usage error and does nothing.

## No other change

Every other command, option, output and exit code stays as in features 002 to 005; the move of
the CLI's code into `runtime::cli` changes no behaviour, and `runtime/tests/cli.rs` passes
unchanged. The real `runtime` ignores `DASHBOARD_TEST_NETWORK`: a new `cli.rs` test sets it to a
live test service and checks that the service gets no request and the output is unchanged.
