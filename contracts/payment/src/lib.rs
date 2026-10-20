#![no_std]

use konfirm_common::{require_admin, require_not_paused, set_paused, CommonError};
use soroban_sdk::{contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, Address, BytesN, Env};

// This contract is an attestation layer, not an escrow. Funds for a checkout
// payment move directly payer -> merchant muxed address via a classic
// Stellar payment operation; this contract never holds them. record_payment
// is called by the reconciler only *after* it has independently observed and
// verified that transfer on Horizon — the on-chain record exists because the
// payment already happened, never before.

const TTL_THRESHOLD: u32 = 100;
const TTL_EXTEND_TO: u32 = 5_000_000;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentRecord {
    pub merchant: Address,
    pub payer: Address,
    pub link_id: u64,
    pub amount: i128,
    pub tx_hash: BytesN<32>,
    pub refunded: bool,
}

#[contractevent]
#[derive(Clone, Debug, PartialEq)]
pub struct InitEvent {
    #[topic]
    pub admin: Address,
}

#[contractevent]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentRecordedEvent {
    #[topic]
    pub merchant: Address,
    pub id: u64,
    pub link_id: u64,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentRefundedEvent {
    pub payment_id: u64,
}

#[contractevent]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentVoidedEvent {
    pub payment_id: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Paused,
    NextId,
    Payment(u64),
    Voided(u64),
    ByTxHash(BytesN<32>),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    /// Never returned: admin gates use `require_auth()`, which traps at the host before this can be produced. Code 3 stays reserved for ABI stability.
    Unauthorized = 3,
    Paused = 4,
    NotFound = 5,
    InvalidAmount = 6,
    AlreadyRefunded = 7,
    AlreadyVoided = 8,
    Voided = 9,
    DuplicateTxHash = 10,
}

#[contract]
pub struct PaymentContract;

#[contractimpl]
impl PaymentContract {
    /// One-time setup. Sets the admin, starts unpaused, and sets the first
    /// payment id to 1. Requires the admin's auth; not pause-gated (there is
    /// no pause state yet). Fails with `AlreadyInitialized` on a second call.
    /// See `docs/ERRORS.md`.
    pub fn initialize(env: Env, admin: Address) -> Result<(), PaymentError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PaymentError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::NextId, &1u64);
        InitEvent { admin: admin.clone() }.publish(&env);
        Ok(())
    }

    // Requires the stored admin's auth; `NotInitialized` before `initialize`.
    fn require_admin(env: &Env) -> Result<(), PaymentError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PaymentError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    // Fails with `Paused` while the contract is paused.
    fn require_not_paused(env: &Env) -> Result<(), PaymentError> {
        let paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
        if paused {
            return Err(PaymentError::Paused);
        }
        Ok(())
    }

    /// Admin-only, pause-gated. Writes an attestation that a payer -> merchant
    /// transfer already happened, and returns the new payment id. Does not
    /// move funds. The reconciler calls this only after verifying the transfer
    /// on Horizon.
    ///
    /// - `merchant`: the address that received the funds.
    /// - `payer`: the address the funds came from.
    /// - `link_id`: the off-chain payment link this payment settles.
    /// - `amount`: the transferred amount in the asset's base units; must be `> 0`.
    /// - `tx_hash`: hash of the Stellar transaction that moved the funds. Each
    ///   hash can be recorded once.
    ///
    /// Fails with `InvalidAmount` if `amount <= 0`, `DuplicateTxHash` if
    /// `tx_hash` was already recorded, and `Paused` while paused.
    pub fn record_payment(
        env: Env,
        merchant: Address,
        payer: Address,
        link_id: u64,
        amount: i128,
        tx_hash: BytesN<32>,
    ) -> Result<u64, PaymentError> {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Self::require_admin(&env)?;
        Self::require_not_paused(&env)?;
        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }
        if env.storage().persistent().has(&DataKey::ByTxHash(tx_hash.clone())) {
            return Err(PaymentError::DuplicateTxHash);
        }

        let id: u64 = env.storage().instance().get(&DataKey::NextId).unwrap_or(1);
        env.storage().instance().set(&DataKey::NextId, &(id + 1));

        let record = PaymentRecord {
            merchant: merchant.clone(),
            payer,
            link_id,
            amount,
            tx_hash: tx_hash.clone(),
            refunded: false,
        };
        env.storage().persistent().set(&DataKey::Payment(id), &record);
        env.storage().persistent().extend_ttl(&DataKey::Payment(id), TTL_THRESHOLD, TTL_EXTEND_TO);
        env.storage().persistent().set(&DataKey::ByTxHash(tx_hash.clone()), &id);
        env.storage().persistent().extend_ttl(&DataKey::ByTxHash(tx_hash), TTL_THRESHOLD, TTL_EXTEND_TO);
        PaymentRecordedEvent { merchant: merchant.clone(), id, link_id, amount }.publish(&env);
        Ok(id)
    }

    /// Admin-only, pause-gated. Flags an existing payment as refunded.
    /// Bookkeeping only: the refund transfer itself is not made by this
    /// contract. Fails with `NotFound` for an unknown id, `AlreadyRefunded` if
    /// already flagged, `Voided` if the payment was voided, and `Paused` while
    /// paused.
    pub fn mark_refunded(env: Env, payment_id: u64) -> Result<(), PaymentError> {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Self::require_admin(&env)?;
        Self::require_not_paused(&env)?;
        let mut record: PaymentRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Payment(payment_id))
            .ok_or(PaymentError::NotFound)?;
        if record.refunded {
            return Err(PaymentError::AlreadyRefunded);
        }
        if Self::is_voided_flag(&env, payment_id) {
            return Err(PaymentError::Voided);
        }
        record.refunded = true;
        env.storage().persistent().set(&DataKey::Payment(payment_id), &record);
        env.storage().persistent().extend_ttl(&DataKey::Payment(payment_id), TTL_THRESHOLD, TTL_EXTEND_TO);
        PaymentRefundedEvent { payment_id }.publish(&env);
        Ok(())
    }

    /// Admin-only, pause-gated. Marks a record as mis-recorded: no money moved,
    /// so nothing is reversed on the ledger. The record itself is never
    /// rewritten; the voided flag sits alongside it and consumers must check
    /// `is_voided` before trusting it. Fails with `NotFound`, `AlreadyVoided`,
    /// `AlreadyRefunded`, or `Paused` while paused.
    pub fn void_payment(env: Env, payment_id: u64) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        Self::require_not_paused(&env)?;
        let record: PaymentRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Payment(payment_id))
            .ok_or(PaymentError::NotFound)?;
        if Self::is_voided_flag(&env, payment_id) {
            return Err(PaymentError::AlreadyVoided);
        }
        if record.refunded {
            return Err(PaymentError::AlreadyRefunded);
        }
        env.storage().persistent().set(&DataKey::Voided(payment_id), &true);
        PaymentVoidedEvent { payment_id }.publish(&env);
        Ok(())
    }

    // Raw voided flag; `false` for unknown ids. Callers check existence first.
    fn is_voided_flag(env: &Env, payment_id: u64) -> bool {
        env.storage().persistent().get(&DataKey::Voided(payment_id)).unwrap_or(false)
    }

    /// Permissionless. Read-only and not pause-gated. Returns whether the
    /// payment was voided, or

/* … truncated 6926 chars — edit only what you need near the top … */
