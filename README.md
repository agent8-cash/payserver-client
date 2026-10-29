# payserver-client

The web frontend for the random.cash payservers. One client, any chain.

It was extracted from [ethpayserver](https://github.com/gcannelloni/ethpayserver),
where it lived as `client/`, with its history intact — `git log` here goes back
to the first commit of the frontend in January 2026.

## Why it is its own repository

The plan is one frontend across several payservers — EVM, Tron, Solana, Monero,
and Bitcoin if it is not simply delegated to BTCPay — each of them a separate
backend, and a self-hoster running whichever subset they actually want. A
frontend that lives inside one of those backends is a frontend that quietly
grows dependencies on it. This one had exactly one such dependency by the end
(`evm`, for a single test), and removing it was the last step before this repo
could exist.

So the rule here is structural, not aspirational: **this repository has no
`path` dependency on any payserver, and cannot acquire one.** Everything shared
arrives through [payserver-commons](https://github.com/gcannelloni/payserver-commons),
pinned by revision in `Cargo.toml`. If the client needs to know something about
a chain, that knowledge belongs in commons — as a CAIP-2 identifier, an
`api-types` shape, a `ui-kit` component — where every payserver reads the same
definition.

## Layout

```
src/            the Leptos app
index.html      trunk entry point; carries the error telemetry DSN meta tags
styles.css
docker/         nginx image that serves the built bundle
scripts/        commons.sh - pin/link the shared crates
```

## Build

```sh
cargo install trunk --locked
trunk build --release          # -> dist/
trunk serve                    # dev server on :8080, proxying /api to :3000
```

`trunk serve` proxies `/api/` to `http://127.0.0.1:3000`, so run a payserver
there and the client talks to it unchanged.

## Working on commons at the same time

`Cargo.toml` pins payserver-commons by revision, which is deliberately
inconvenient when you are editing both. `scripts/commons.sh` is the way out:

```sh
scripts/commons.sh status          # what is pinned, and whether a link is live
scripts/commons.sh link [path]     # build against a local checkout (default: ../payserver-commons)
scripts/commons.sh unlink          # back to the pinned revision
scripts/commons.sh pin <rev|main>  # move the pin
```

`link` writes an **uncommitted** `.cargo/config.toml`. Always `unlink` before
committing — a `Cargo.lock` recorded under a link points at your local path
instead of a revision, which un-pins commons for everyone and looks like
nothing. CI fails the build if the lock and the pin disagree.

## Bundle size

The bundle is built with `opt-level = "z"` and `wasm-opt -Oz`, and CI prints the
size of every artifact on each build. This is why, for instance, secret
redaction (the `scrub` crate in commons) is hand-written scanners rather than
regexes: pulling in `regex` costs about 1 MB of WASM, measured 2.28 MB → 3.28 MB.
Weigh new dependencies accordingly.

## Error reporting

Off unless configured. `index.html` carries `telemetry-dsn` and
`telemetry-environment` meta tags, read out of the DOM at page load rather
than compiled into the WASM binary — so setting them needs no Rust rebuild,
only an edit to `index.html` before CI builds it in. There is no deploy-time
templating step that fills these in today; the served value is whatever is
committed. The collector is not reachable from
the public internet, so an end-user browser cannot reach it — this is for
internal and dev traffic. Panics, uncaught errors and
unhandled rejections are sent; never cookies, request bodies, user identity or
query strings. Every free-text field goes through `scrub::redact_secrets`, the
same implementation the payservers run. See `src/telemetry`.

## Verifying the image

The published image is signed with cosign in keyless mode — no private key
exists; the signer is this repository's GitHub Actions job, recorded in the
public Sigstore transparency log. Signatures are made against the **digest**,
not the tag.

```sh
cosign verify \
  --certificate-identity-regexp \
    "^https://github.com/randomcash/payserver-client/\.github/workflows/ci\.yml@refs/(heads|tags)/.*$" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  ghcr.io/randomcash/payserver-client:sha-abc1234
```

A payserver pins a specific tag of this image and verifies it under this
identity — which is not the payserver's own, since the frontend is built here.

## Licence

MIT. See [LICENSE](LICENSE).
