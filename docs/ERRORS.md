# Error Codes

Every contract exposes a `#[contracterror]` enum. Soroban returns the numeric
`u32` code in the `Error(Contract, #N)` result, so callers should match on the
code rather than the variant name. This file is the canonical reference for
those codes.

When you add, remove, or rename a variant in any `#[contracterror]` enum, update
the matching table below. `scripts/check_error_docs.py` fails CI if a variant is
missing from this file.

## payment (`contracts/payment`)

| Code | Variant | Meaning |
| ---- | ------- | ------- |
| 1 | `AlreadyInitialized` | `initialize` was called more than once. |
| 2 | `NotInitialized` | The contract has not been initialized yet. |
| 3 | `Unauthorized` | Caller is not the admin/owner for this operation. |
| 4 | `Paused` | The contract is paused; mutating calls are rejected. |
| 5 | `NotFound` | The referenced payment does not exist. |
| 6 | `InvalidAmount` | Amount is zero or otherwise invalid. |
| 7 | `AlreadyRefunded` | The payment has already been refunded. |
| 8 | `AlreadyVoided` | The payment has already been voided. |
| 9 | `Voided` | The payment was voided; the operation is not allowed. |

## treasury (`contracts/treasury`)

| Code | Variant | Meaning |
| ---- | ------- | ------- |
| 1 | `AlreadyInitialized` | `initialize` was called more than once. |
| 2 | `NotInitialized` | The contract has not been initialized yet. |
| 3 | `Unauthorized` | Caller is not the admin/owner for this operation. |
| 4 | `Paused` | The contract is paused; mutating calls are rejected. |
| 5 | `NotFound` | The referenced treasury entry does not exist. |
| 6 | `InvalidAmount` | Amount is zero or otherwise invalid. |
| 7 | `InsufficientFunds` | The treasury does not hold enough funds. |
| 8 | `InvalidSigner` | A signer address is invalid. |
| 9 | `NotASigner` | The address is not a registered signer. |
| 10 | `AlreadySigned` | This signer has already signed the proposal. |
| 11 | `InvalidThreshold` | The signer threshold is invalid. |
| 12 | `DuplicateSigner` | `initialize` was given the same signer address twice. |

## channel (`contracts/channel`)

| Code | Variant | Meaning |
| ---- | ------- | ------- |
| 1 | `AlreadyInitialized` | `initialize` was called more than once. |
| 2 | `NotInitialized` | The contract has not been initialized yet. |
| 3 | `Unauthorized` | Caller is not the admin/owner for this operation. |
| 4 | `Paused` | The contract is paused; mutating calls are rejected. |
| 5 | `NotFound` | The referenced channel does not exist. |
| 6 | `InvalidAmount` | Amount is zero or otherwise invalid. |
| 7 | `ChannelClosed` | The channel is already closed. |
| 8 | `ChannelNotOpen` | The channel is not open. |
| 9 | `InsufficientBalance` | The channel does not hold enough balance. |
| 10 | `InvalidState` | The channel is in an invalid state for this operation. |
| 11 | `InvalidSignature` | A signature failed verification. |
| 12 | `ZeroDeposit` | The deposit amount is zero. |
| 13 | `TokenNotAllowed` | The token is not on the admin allowlist. |

## compliance (`contracts/compliance`)

| Code | Variant | Meaning |
| ---- | ------- | ------- |
| 1 | `AlreadyInitialized` | `initialize` was called more than once. |
| 2 | `NotInitialized` | The contract has not been initialized yet. |
| 3 | `Unauthorized` | Caller is not the admin/owner for this operation. |
| 4 | `NotFound` | The referenced compliance record does not exist. |
