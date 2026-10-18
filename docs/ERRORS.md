# Error codes

Each contract reports failures as a `#[contracterror]` enum. On-chain, a failed
call surfaces as `Error(Contract, #N)`, where `N` is the numeric value in the
tables below. Codes are per contract: `#1` in `payment` and `#1` in `treasury`
are different errors, so look up the code under the contract you called.

Calls that fail `require_auth` do not return one of these codes. The host
traps the invocation instead, so an auth failure shows up as a host error, not
as `Unauthorized`. `Unauthorized` is declared in every enum but no code path in
the current contracts returns it.

## payment (`PaymentError`)

| Code | Variant | Trigger |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` called after the contract was already set up. |
| 2 | `NotInitialized` | An admin-only call (`record_payment`, `mark_refunded`, `pause`, `unpause`) made before `initialize`. |
| 3 | `Unauthorized` | Declared but not returned. Auth failures trap (see above). |
| 4 | `Paused` | `record_payment` or `mark_refunded` while the contract is paused. |
| 5 | `NotFound` | `get_payment` or `mark_refunded` with a payment id that does not exist. |
| 6 | `ZeroAmount` | `record_payment` with `amount <= 0`. |
| 7 | `AlreadyRefunded` | `mark_refunded` on a payment that is already flagged as refunded. |

## treasury (`TreasuryError`)

| Code | Variant | Trigger |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` called after the contract was already set up. |
| 2 | `NotInitialized` | A call that needs stored config (admin, token, signers, threshold) made before `initialize`. |
| 3 | `Unauthorized` | Declared but not returned. Auth failures trap (see above). |
| 4 | `Paused` | `propose_settlement`, `approve_settlement` or `execute_settlement` while paused. |
| 5 | `NotFound` | `approve_settlement`, `execute_settlement` or `get_settlement` with an unknown settlement id. |
| 6 | `ZeroAmount` | `propose_settlement` with `amount <= 0`. |
| 7 | `NotSigner` | `propose_settlement` or `approve_settlement` by an address not in the signer set. |
| 8 | `AlreadyApproved` | A signer approves the same settlement a second time. The proposer's approval counts from the start. |
| 9 | `ThresholdNotMet` | `execute_settlement` before the approval count reaches the threshold. |
| 10 | `NotPending` | `approve_settlement` or `execute_settlement` on a settlement that is already `Executed` or `Cancelled`. |
| 11 | `InvalidThreshold` | `initialize` with a threshold of `0` or greater than the number of signers. |

## channel (`ChannelError`)

| Code | Variant | Trigger |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` called after the contract was already set up. |
| 2 | `NotInitialized` | A call that needs stored config (admin, challenge period) made before `initialize`. |
| 3 | `Unauthorized` | `top_up` by someone other than the channel's payer, or `initiate_close` by someone other than the payer or payee. |
| 4 | `Paused` | `open_channel` or `top_up` while paused. `checkpoint`, `initiate_close` and `finalize_close` are never blocked by pause. |
| 5 | `NotFound` | Any call with a channel id that does not exist. |
| 6 | `NotOpen` | `top_up` or `initiate_close` on a channel that is not `Open`; `checkpoint` on a `Closed` channel; `release_hold` on a channel that is not `Held`. |
| 7 | `StaleClaim` | `checkpoint` with a `nonce` not greater than the stored one, or a `cumulative_amount` not greater than what was already claimed. |
| 8 | `ExceedsDeposit` | `checkpoint` with a `cumulative_amount` greater than the channel's deposit. |
| 9 | `ChallengeNotElapsed` | `finalize_close` before `closing_at + challenge period`. |
| 10 | `NotClosing` | `finalize_close` on a channel that is not `Closing`. |
| 11 | `Held` | `checkpoint` on a channel that is `Held` by admin. |
| 12 | `ZeroDeposit` | `open_channel` or `top_up` with an amount `<= 0`. |

An invalid payer signature in `checkpoint` traps the invocation (the Ed25519
check is a host function) and does not return a code.

## compliance (`ComplianceError`)

| Code | Variant | Trigger |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` called after the contract was already set up. |
| 2 | `NotInitialized` | An admin-only call (`block_address`, `allow_address`, `clear_address`, `pause`, `unpause`) made before `initialize`. |
| 3 | `Unauthorized` | Declared but not returned. Auth failures trap (see above). |
| 4 | `Paused` | `block_address`, `allow_address` or `clear_address` while paused. `is_allowed` and `pause`/`unpause` are not blocked. |

`is_allowed` is read-only and never returns an error. It returns `true` unless
the address is on the block list.
