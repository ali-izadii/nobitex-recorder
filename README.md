# Nobitex Recorder

A Rust learning project for watching one Nobitex market at a time, recording
public order-book and trade events, and replaying recordings through the same
analysis pipeline. Start with `BTCIRT`; public market data needs no account.

The [project plan](.agents/doc/nobitex-market-recorder-and-replay.md) describes
the learning goals and build steps. Recording and replay are planned features.

Currently, the snapshot command displays the best bid, best ask, spread
(ask minus bid), exchange update timestamp, and local receipt time. Prices and
quantities use exact decimal arithmetic. Missing prices display as `N/A`.
Price values are currently shown in raw API units; quote-currency units and
timestamp encoding still need verification.

## Setup

Install a current stable Rust toolchain supporting edition 2024. Create
`config.toml` in the project root:

```toml
nobitex_base_url = "https://apiv2.nobitex.ir"
timeout_second = 15
api_key = ""
```

Public snapshots need no API key. The empty field is required by the current
configuration parser.

## Usage

Run from the project root so the program can find `config.toml`:

```bash
cargo run -- snapshot
cargo run -- snapshot --symbol ETHUSDT
cargo run -- --help
```

The default market is `BTCIRT`. The CLI also accepts other market symbols,
one per invocation.

## Build path

1. Define commands, output units, spread, and missing-data behavior.
2. Parse and validate a REST snapshot with exact decimals.
3. Subscribe to a public WebSocket order-book channel, then add trades.
4. Separate transport, normalization, analysis, and persistence.
5. Record payloads, timestamps, and session events faithfully.
6. Replay in arrival order and compare deterministic results.
7. Handle reconnects and mark recording gaps explicitly.

## Status

- REST snapshot fetching, decimal parsing, and level validation are implemented.
- Step 2 protocol verification remains open: inspect another market and confirm
  price units and timestamp encoding.
- Live watching, recording, and replay are not implemented yet. The current
  CLI includes placeholder `watch` and `reply` commands.
- Automated tests are deferred to a later step by the developer; the plan's
  fixture-based validation has not yet been demonstrated.

Check compilation with `cargo check --locked`.
