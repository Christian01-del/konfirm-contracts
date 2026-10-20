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
        let threshold: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Threshold)
            .ok_or(TreasuryError::NotInitialized)?;
        if settlement.approvals.len() < threshold {
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

    /// Admin-only. Pauses or unpauses the contract. While paused,
    /// `propose_settlement`, `approve_settlement` and `execute_settlement`
    /// return `Paused`; `initialize` is unaffected.
    pub fn set_paused(env: Env, paused: bool) -> Result<(), TreasuryError> {
        Self::require_admin(&env)?;
        set_paused(&env, &DataKey::Paused, paused);
        Ok(())
    }

    /// Read-only. Returns the stored approval threshold, or
    /// `NotInitialized` before `initialize`.
    pub fn get_threshold(env: Env) -> Result<u32, TreasuryError> {
        env.storage()
            .instance()
            .get(&DataKey::Threshold)
            .ok_or(TreasuryError::NotInitialized)
    }

    /// Read-only. Returns the stored signer set, or `NotInitialized` before
    /// `initialize`.
    pub fn get_signers(env: Env) -> Result<Vec<Address>, TreasuryError> {
        env.storage()
            .instance()
            .get(&DataKey::Signers)
            .ok_or(TreasuryError::NotInitialized)
    }

    /// Read-only. Returns the settlement with `settlement_id`, or `NotFound`.
    pub fn get_settlement(env: Env, settlement_id: u64) -> Result<Settlement, TreasuryError> {
        env.storage()
            .persistent()
            .get(&DataKey::Settlement(settlement_id))
            .ok_or(TreasuryError::NotFound)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::testutils::Address as _;

    // Property 1: for any signer-set size N and threshold T, initialize
    // succeeds iff 1 <= T <= N. This directly exercises the T == N boundary
    // (e.g. 3-of-3) that the `>` -> `>=` mutant would otherwise survive.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn threshold_bounds_match_signer_count(n in 1usize..=8, t in 0u32..=10) {
            let env = Env::default();
            env.mock_all_auths();
            let contract_id = env.register_contract(None, TreasuryContract);
            let client = TreasuryContractClient::new(&env, &contract_id);

            let admin = Address::generate(&env);
            let token = Address::generate(&env);
            let mut signers = Vec::new(&env);
            for _ in 0..n {
                signers.push_back(Address::generate(&env));
            }

            let result = client.try_initialize(&admin, &token, &signers, &t);
            let expected_ok = t >= 1 && (t as usize) <= n;
            prop_assert_eq!(result.is_ok(), expected_ok);
        }
    }

    // Property 1 (continued): approval counting never double-counts a single
    // signer. For any threshold T and any sequence of approval attempts
    // (with repeats), the settlement only reaches the threshold once at
    // least T *distinct* signers have approved.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn approvals_count_distinct_signers(
            n in 1usize..=6,
            t in 1u32..=6,
            attempts in proptest::collection::vec(0usize..6, 0..12),
        ) {
            prop_assume!(t as usize <= n);
            let env = Env::default();
            env.mock_all_auths();
            let contract_id = env.register_contract(None, TreasuryContract);
            let client = TreasuryContractClient::new(&env, &contract_id);

            let admin = Address::generate(&env);
            let token = Address::generate(&env);
            let mut signers = Vec::new(&env);
            for _ in 0..n {
                signers.push_back(Address::generate(&env));
            }
            client.initialize(&admin, &token, &signers, &t);

            let merchant = Address::generate(&env);
            let proposer = signers.get(0).unwrap();
            let id = client.propose_settlement(&proposer, &merchant, &100i128);

            let mut distinct: Vec<Address> = Vec::new(&env);
            distinct.push_back(proposer.clone());

            for idx in attempts.iter() {
                let signer = signers.get((*idx as u32) % (n as u32)).unwrap();
                let res = client.try_approve_settlement(&signer, &id);
                if !distinct.contains(&signer) {
                    // First approval by this signer must succeed.
                    prop_assert!(res.is_ok());
                    distinct.push_back(signer);
                } else {
                    // Repeat approval must be rejected as AlreadyApproved.
                    prop_assert_eq!(res, Err(Ok(TreasuryError::AlreadyApproved)));
                }
            }

            let settlement = client.get_settlement(&id);
            prop_assert_eq!(settlement.approvals.len(), distinct.len());
            prop_assert!(settlement.approvals.len() <= n as u32);
        }
    }
}
