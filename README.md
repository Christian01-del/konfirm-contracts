# konfirm-contracts

Four Soroban smart contracts backing [Konfirm](https://github.com/konfirm-HQ/konfirm-backend), a non-custodial payment processor on Stellar. All four are deployed to Testnet (addresses below); none are currently called by the live checkout or cash-out flow in `konfirm-backend` — today's pilot moves value with plain Stellar classic payments, and these contracts are the on-chain layer that path is built toward.

## Branch protection

`main` is protected. The rule is declared in [`.github/branch-protection.json`](.github/branch-protection.json) and applied with:

```bash
gh api -X PUT repos/Konfirm-xyz/konfirm-contracts/branches/main/protection \
  --input .github/branch-protection.json
```

It requires pull requests with at least one approving review, requires the CI `test` and `audit` status checks to be green (and up to date with `main`), and blocks force-pushes and branch deletion. Verify with `gh api repos/Konfirm-xyz/konfirm-contracts/branches/main/protection` (expects 200).

## Contracts

| Contract | Purpose |
|---|---|
| `compliance` | Default-allow address screening with an explicit deny/allow list, pausable |
| `payment` | Attestation layer — records that a payment happened and its refund state; not an escrow |
| `treasury` | 2-of-3 multisig settlement: propose → approve → execute |
| `channel` | Escrow-backed payment channels with checkpointing, a configurable challenge period, and unilateral close |

Every contract follows the same shape: an `initialize` entry point, a `pause`/`unpause` pair, and mutations that check the pause flag before touching state.

### `compliance`

| Function | Signature |
|---|---|
| `initialize` | `(admin: Address)` |
| `is_allowed` | `(addr: Address) -> bool` |
| `block_address` / `allow_address` / `clear_address` | `(addr: Address)` |
| `set_admin` | `(current_admin: Address, new_admin: Address)` — both must authorize; not pause-gated |
| `pause` / `unpause` | `()` |

Default-allow: an address is permitted unless explicitly blocked. `clear_address` restores the default rather than leaving an explicit "allowed" entry.

**Advisory, not enforced on-chain.** `compliance` is consulted off-chain by `konfirm-backend` (`isAllowedOnChain`) before it calls `payment::record_payment` or proposes a `treasury` settlement. The `payment`, `treasury`, and `channel` contracts never call `compliance`, so a blocked address is not rejected by them. Treat `compliance` as the screening source of truth for the backend, not as a hard on-chain gate. Wiring it in as a gate would mean passing the compliance contract ID into `payment` and `treasury` at initialization and checking `is_allowed` in `record_payment`, `propose_settlement`, and `execute_settlement`.

### `payment`

| Function | Signature |
|---|---|
| `initialize` | `(admin: Address)` |
| `record_payment` | see `lib.rs` — writes a payment record |
| `mark_refunded` | `(payment_id: u64)` — rejects a second refund on the same ID |
| `get_payment` | `(payment_id: u64) -> PaymentRecord` |
| `set_admin` | `(current_admin: Address, new_admin: Address)` — both must authorize; not pause-gated |
| `pause` / `unpause` | `()` |

### `treasury`

| Function | Signature |
|---|---|
| `initialize` | signer set + threshold |
| `propose_settlement` | creates a pending settlement |
| `approve_settlement` | `(signer: Address, settlement_id: u64)` — rejects non-signers and double-approval |
| `execute_settlement` | `(caller: Address, settlement_id: u64)` — caller must be a signer; only once threshold is met; rejects double-execution |
| `get_settlement` | `(settlement_id: u64) -> Settlement` |
| `pause` / `unpause` | `()` |

Deployed with a 2-of-3 threshold (see [Deployed addresses](#deployed-addresses-testnet)).

### `channel`

| Function | Signature |
|---|---|
| `initialize` | `(admin: Address, challenge_period_secs: u64)` — challenge period is a deployment-time parameter, not hardcoded |
| `open_channel` | opens an escrow-backed channel |
| `top_up` | `(payer: Address, channel_id: u64, amount: i128)` |
| `checkpoint` | signed state update; rejects stale nonces |
| `initiate_close` / `finalize_close` | unilateral close, respecting the challenge period; refunds the remainder once it elapses |
| `hold_channel` / `release_hold` | freezes/unfreezes checkpointing without closing |
| `get_channel_info` | `(channel_id: u64) -> Channel` |
| `set_admin` | `(current_admin: Address, new_admin: Address)` — both must authorize; not pause-gated |
| `pause` / `unpause` | `()` |

Signed messages are domain-separated with `DOMAIN_TAG = b"KONFIRM_CHAN_V1"` (15 bytes) prefixed onto the signing payload, so a signature from this contract can never be replayed against a different one.

The deployed instance uses `challenge_period_secs = 86400` (24 hours). Shorter/longer periods for lower/higher-value channels are a deployment decision, not a contract feature. The contract enforces one constraint only: the value must be greater than zero. `initialize` returns `InvalidChallengePeriod` for `0`. A zero period would let `finalize_close` succeed in the same ledger as `initiate_close`, which leaves the payee no window to checkpoint a higher claim.

**Intentionally unused by the current pilot:** `top_up`, `hold_channel`, and `release_hold` are not called by `konfirm-backend` or `konfirm-frontend` today — the pilot's happy path only needs `open_channel`/`checkpoint`/`initiate_close`/`finalize_close`/`get_channel_info`. They stay in the deployed contract as the on-chain surface for features that will need them: `top_up` when a channel must accept additional deposit capacity beyond its opening balance, and `hold_channel`/`release_hold` when dispute or hold semantics (freezing checkpointing without closing) land in the backend. Wire them up at that point rather than removing them now.

## Deployed addresses (Testnet)

Each address below was built from the commit recorded in the **Commit** column — the short SHA of `HEAD` at deploy time. To answer "which commit's wasm is this address running?", read the hash here (or the matching `commit` field in `artifacts/addresses.json`) and check it out; no `git log` archaeology required.

| Contract | Address | Commit |
|---|---|---|
| Compliance | `CDDVLE2DZQAYFY3Z2Z74TUNNPC4ROUACSBXOB2P64IT75EZFAQXSRSXY` | `7a4b664` |
| Payment | `CCYRA6JT2L4NS5FG4B5TP52JPCGCPYSP7M6LUDUY2QA37V5UBXWJBRHV` | `7a4b664` |
| Treasury | `CD77HPVBGIRYQGXC4JVCEO35X6FKFFJ2C4EZ63EQCOXGR6OL4TVEPZ2T` | `7a4b664` |
| Channel | `CDS2Y4CQMQWFLCG5GHVKX7UIXHYPM6IJDJZTEXSASSHGHLESGLGLNPL6` | `7a4b664` |

Deployer: `GAEMG5TVLEIQYCY3XB4EJT742DIE3FQO53RSESSYJQUZIWZOJQIZATJS`

Treasury signers (2-of-3 threshold): `GAEMG5TVLEIQYCY3XB4EJT742DIE3FQO53RSESSYJQUZIWZOJQIZATJS`, `GBRUR4UZHKPQ76S4S7X7INENL6QJ4UGNFIQ3F6VAYJQZRO3F4XBZAIND`, `GCP57AJNZIVVTPPSD4MJ2SQDMTN4QEAUP64U2OZ4HXSAU4O4A6NOWY2Z`

### Recording provenance on redeploy

Whenever a contract is redeployed, record the commit it was built from so the mapping above never rots:

1. Note the short SHA of `HEAD` at deploy time: `git rev-parse --short HEAD`.
2. Update the **Commit** column for the redeployed contract(s) in the table above.
3. Update the matching `commit` field next to that network's addresses in `artifacts/addresses.json`.
4. Optionally tag the deploy (`git tag testnet-deploy-YYYY-MM-DD`) so the exact build is also reachable by tag.

## Tech stack

- Rust, `soroban-sdk 27.0.5`
- `#![no_std]` contracts — no heap allocation, no std library
- 27 unit tests across the four contracts using `soroban-sdk`'s `testutils`

## Prerequisites

- Rust (stable)
- The `wasm32v1-none` target — **not** `wasm32-unknown-unknown`. `rust-toolchain.toml` currently lists `wasm32-unknown-unknown`, but `soroban-sdk 27` rejects that target on Rust 1.82+; install the correct one explicitly:

  ```bash
  rustup target add wasm32v1-none
  ```
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) (`stellar` on `$PATH`) for building and deploying

## Build & test

```bash
cargo test --all --features testutils
```

The `testutils` feature must be enabled. Plain `cargo test` fails to compile, because the test code imports `soroban_sdk::testutils`. CI runs the command above.

```bash
stellar contract build
```

Current `stellar-cli` (27.x) bakes in the `wasm32v1-none` target for `contract build` and rejects a `--target` flag, so no target argument is needed.

Deploy, then initialize. These contracts have **no constructor**: passing `-- --admin ...` at deploy time does not set them up. The `initialize` call is separate. The `channel` example:

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/konfirm_channel.wasm \
  --source deployer --network testnet
# -> prints the contract id, <CONTRACT_ID>

stellar contract invoke --id <CONTRACT_ID> \
  --source deployer --network testnet \
  -- initialize --admin <admin_address> --challenge_period_secs 86400
```

Each contract's `initialize` takes its own arguments: `compliance` and `payment` take `--admin`; `treasury` takes `--admin --token --signers --threshold`; `channel` takes `--admin --challenge_period_secs`. Calling it twice fails with `AlreadyInitialized`, which is the protection.

Verified on testnet: a deploy without `initialize` leaves every admin call failing with `NotInitialized` (error 2).

## Authorization convention

Every `pub fn` in `contracts/*/src/lib.rs` must do one of two things. CI enforces this with `python3 scripts/check_pub_fn_auth.py`, and the check fails the build otherwise:

- **Autho
