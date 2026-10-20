#![no_std]

use konfirm_common::{require_not_paused, set_paused, CommonError};
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, token, vec, Address, Env, Vec};

const TTL_THRESHOLD: u32 = 100;
const TTL_EXTEND_TO: u32 = 5_000_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettlementStatus {
    Pending,
    Executed,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct Settlement {
    pub merchant: Address,
    pub amount: i128,
    pub approvals: Vec<Address>,
    // Parallel to `approvals`: approved_at[i] is the ledger timestamp at which approvals[i] signed.
    pub approved_at: Vec<u64>,
    pub status: SettlementStatus,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Token,
    Paused,
    Signers,
    Threshold,
    NextId,
    Settlement(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TreasuryError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    /// Never returned: admin/signer gates use `require_auth()`, which traps at the host before this can be produced. Code 3 stays reserved for ABI stability.
    Unauthorized = 3,
    Paused = 4,
    NotFound = 5,
    ZeroAmount = 6,
    NotSigner = 7,
    AlreadyApproved = 8,
    ThresholdNotMet = 9,
    NotPending = 10,
    InvalidThreshold = 11,
    DuplicateSigner = 12,
}

impl From<CommonError> for TreasuryError {
    fn from(err: CommonError) -> Self {
        match err {
            CommonError::NotInitialized => TreasuryError::NotInitialized,
            CommonError::Paused => TreasuryError::Paused,
        }
    }
}

#[contract]
pub struct TreasuryContract;

#[contractimpl]
impl TreasuryContract {
    /// One-time setup. Requires the admin's auth; not pause-gated. Stores the
    /// admin, the payout token, the signer set and the approval threshold —
    /// the number of distinct signer approvals a settlement needs before it
    /// can execute. Fails with `AlreadyInitialized` on a second call,
    /// `InvalidThreshold` unless `1 <= threshold <= signers.len()`, and
    /// `DuplicateSigner` if an address appears twice in `signers`. The
    /// contract holds no funds until it is funded separately;
    /// `execute_settlement` pays out of its balance.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        signers: Vec<Address>,
        threshold: u32,
    ) -> Result<(), TreasuryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(TreasuryError::AlreadyInitialized);
        }
        if threshold == 0 || threshold > signers.len() {
            return Err(TreasuryError::InvalidThreshold);
        }
        for (i, signer) in signers.iter().enumerate() {
            if signers.iter().skip(i + 1).any(|other| other == signer) {
                return Err(TreasuryError::DuplicateSigner);
            }
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::Signers, &signers);
        env.storage().instance().set(&DataKey::Threshold, &threshold);
        env.storage().instance().set(&DataKey::NextId, &1u64);
        env.events().publish((symbol_short!("init"),), admin);
        Ok(())
    }

    // Requires `who`'s auth and that `who` is in the signer set (`NotSigner`).
    fn require_signer(env: &Env, who: &Address) -> Result<(), TreasuryError> {
        who.require_auth();
        let signers: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Signers)
            .ok_or(TreasuryError::NotInitialized)?;
        if !signers.contains(who) {
            return Err(TreasuryError::NotSigner);
        }
        Ok(())
    }

    // Requires the stored admin's auth; `NotInitialized` before `initialize`.
    fn require_admin(env: &Env) -> Result<(), TreasuryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(TreasuryError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    /// Signer-only, pause-gated. Opens a new `Pending` settlement paying
    /// `amount` to `merchant` and returns its id. The proposer's approval
    /// counts as the first of the `threshold` required. Does not move funds.
    /// Fails with `NotSigner`, `ZeroAmount` if `amount <= 0`, and `Paused`
    /// while paused.
    pub fn propose_settlement(
        env: Env,
        proposer: Address,
        merchant: Address,
        amount: i128,
    ) -> Result<u64, TreasuryError> {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Self::require_signer(&env, &proposer)?;
        require_not_paused(&env, &DataKey::Paused)?;
        if amount <= 0 {
            return Err(TreasuryError::ZeroAmount);
        }

        let id: u64 = env.storage().instance().get(&DataKey::NextId).unwrap_or(1);
        env.storage().instance().set(&DataKey::NextId, &(id + 1));

        let settlement = Settlement {
            merchant: merchant.clone(),
            amount,
            approvals: vec![&env, proposer],
            approved_at: vec![&env, env.ledger().timestamp()],
            status: SettlementStatus::Pending,
        };
        env.storage().persistent().set(&DataKey::Settlement(id), &settlement);
        env.storage().persistent().extend_ttl(&DataKey::Settlement(id), TTL_THRESHOLD, TTL_EXTEND_TO);
        env.events().publish((symbol_short!("proposed"), merchant), (id, amount));
        Ok(id)
    }

    /// Signer-only, pause-gated. Adds `signer`'s approval to a `Pending`
    /// settlement. Each signer may approve a given settlement once
    /// (`AlreadyApproved`), so the approval count is the number of distinct
    /// signers. Reaching the threshold does not execute the settlement; that
    /// takes a separate `execute_settlement` call. Fails with `NotSigner`,
    /// `NotFound`, `NotPending`, and `Paused` while paused.
    pub fn approve_settlement(env: Env, signer: Address, settlement_id: u64) -> Result<(), TreasuryError> {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Self::require_signer(&env, &signer)?;
        require_not_paused(&env, &DataKey::Paused)?;

        let mut settlement: Settlement = env
            .storage()
            .persistent()
            .get(&DataKey::Settlement(settlement_id))
            .ok_or(TreasuryError::NotFound)?;
        if settlement.status != SettlementStatus::Pending {
            return Err(TreasuryError::NotPending);
        }
        if settlement.approvals.contains(&signer) {
            return Err(TreasuryError::AlreadyApproved);
        }
        settlement.approvals.push_back(signer);
        settlement.approved_at.push_back(env.ledger().timestamp());
        env.storage().persistent().set(&DataKey::Settlement(settlement_id), &settlement);
        env.storage().persistent().extend_ttl(&DataKey::Settlement(settlement_id), TTL_THRESHOLD, TTL_EXTEND_TO);
        env.events().publish((symbol_short!("approved"),), settlement_id);
        Ok(())
    }

    /// Signer-only, pause-gated. Any one signer (`caller`) can execute once the
    /// settlement's approval count reaches the threshold; fails with
    /// `ThresholdNotMet` before then. Executing does not count as an approval.
    /// Transfers `amount` of the payout token from the treasury to the
    /// merchant, then marks the settlement `Executed`. If the transfer traps,
    /// the whole call reverts and the settlement stays `Pending`. Also fails
    /// with `NotSigner`, `NotFound`, `NotPending`, and `Paused` while paused.
    pub fn execute_settlement(env: Env, caller: Address, settlement_id: u64) -> Result<(), TreasuryError> {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Self::require_signer(&env, &caller)?;
        require_not_paused(&env, &DataKey::Paused)?;

        let mut settlement: Settlement = env
            .storage()
            .persistent()
            .get(&DataKey::Settlement(settlement_id))
            .ok_or(TreasuryError::NotFound)?;
        if settlement.status != SettlementStatus::Pending {
            return Err(TreasuryError::NotPending);
        }
        let threshold: u32 = env.storage().instance().get(&DataKey::Threshold).unwrap_or(0);
        if (settlement.approvals.len() as u32) < threshold {
            return Err(TreasuryError::ThresholdNotMet);
        }

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(TreasuryError::NotInitialized)?;
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(&env.current_contract_address(), &settlement.merchant, &settlement.amount);

        settlement.status = SettlementStatus::Executed;
        env.storage().persistent().set(&DataKey::Settlement(settlement_id), &settlement);
        env.storage().persistent().extend_ttl(&DataKey::Settlement(settlement_id), TTL_THRESHOLD, TTL_EXTEND_TO);
        env.events().publish((symbol_short!("executed"),), settlement_id);
        Ok(())
    }

    /// Signer-only. Cancels a `Pending` settlement. Fails with `NotSigner`,
    /// `NotFound`, and `NotPending`.
    pub fn cancel_settlement(env: Env, signer: Address, settlement_id: u64) -> Result<(), TreasuryError> {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Self::require_signer(&env, &signer)?;

        let mut settlement: Settlement = env
            .storage()
            .persistent()
            .get(&DataKey::Settlement(settlement_id))
            .ok_or(TreasuryError::NotFound)?;
        if settlement.status != SettlementStatus::Pending {
            return Err(TreasuryError::NotPending);
        }
        settlement.status = SettlementStatus::Cancelled;
        env.storage().persistent().set(&DataKey::Settlement(settlement_id), &settlement);
        env.storage().persistent().extend_ttl(&DataKey::Settlement(settlement_id), TTL_THRESHOLD, TTL_EXTEND_TO);
        env.events().publish((symbol_short!("cancelled"),), settlement_id);
        Ok(())
    }

    /// Signer-only. Freezes the contract: `propose_settlement`,
    /// `approve_settlement` and `execute_settlement` all fail with `Paused`
    /// until `unpause`. Gated by `require_signer` (not the single admin key),
    /// so no single key can unilaterally pause-DoS the treasury. Fails with
    /// `NotSigner`.
    pub fn pause(env: Env, signer: Address) -> Result<(), TreasuryError> {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Self::require_signer(&env, &signer)?;
        set_paused(&env, &DataKey::Paused, true)?;
        env.events().publish((symbol_short!("paused"),), signer);
        Ok(())
    }

    /// Signer-only. Resumes settlement flow after a `pause`. Gated by
    /// `require_signer` (not the single admin key). Fails with `NotSigner`.
    pub fn unpause(env: Env, signer: Address) -> Result<(), TreasuryError> {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Self::require_signer(&env, &signer)?;
        set_paused(&env, &DataKey::Paused, false)?;
        env.events().publish((symbol_short!("unpaused"),), signer);
        Ok(())
    }

    /// Returns whether the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    /// Returns the stored admin address.
    pub fn admin(env: Env) -> Result<Address, TreasuryError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(TreasuryError::NotInitialized)
    }

    /// Returns the configured signer set.
    pub fn signers(env: Env) -> Result<Vec<Address>, TreasuryError> {
        env.storage()
            .instance()
            .get(&DataKey::Signers)
            .ok_or(TreasuryError::NotInitialized)
    }

    /// Returns the configured approval threshold.
    pub fn threshold(env: Env) -> Result<u32, TreasuryError> {
        env.storage()
            .instance()
            .get(&DataKey::Threshold)
            .ok_or(TreasuryError::NotInitialized)
    }

    /// Returns a settlement by id.
    pub fn get_settlement(env: Env, settlement_id: u64) -> Result<Settlement, TreasuryError> {
        env.storage()
            .persistent()
            .get(&DataKey::Settlement(settlement_id))
            .ok_or(TreasuryError::NotFound)
    }
}
