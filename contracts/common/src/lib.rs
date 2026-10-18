#![no_std]

use soroban_sdk::{Address, Env, IntoVal, Val};

// Admin and pause plumbing shared by every contract in the workspace. Each
// contract keeps its own storage keys and its own error enum (error codes are
// part of each contract's on-chain ABI), so these helpers are generic over the
// contract's DataKey and return CommonError, which each contract's error enum
// converts from via `From<CommonError>`.

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CommonError {
    NotInitialized,
    Paused,
}

pub fn require_admin<K>(env: &Env, admin_key: &K) -> Result<Address, CommonError>
where
    K: IntoVal<Env, Val>,
{
    let admin: Address = env
        .storage()
        .instance()
        .get(admin_key)
        .ok_or(CommonError::NotInitialized)?;
    admin.require_auth();
    Ok(admin)
}

/// Permissionless. A read-only guard, not a contract entry point: it checks
/// the pause flag and leaves authorization to the calling contract function.
pub fn require_not_paused<K>(env: &Env, paused_key: &K) -> Result<(), CommonError>
where
    K: IntoVal<Env, Val>,
{
    let paused: bool = env.storage().instance().get(paused_key).unwrap_or(false);
    if paused {
        return Err(CommonError::Paused);
    }
    Ok(())
}

pub fn set_paused<K>(env: &Env, admin_key: &K, paused_key: &K, paused: bool) -> Result<(), CommonError>
where
    K: IntoVal<Env, Val>,
{
    require_admin(env, admin_key)?;
    env.storage().instance().set(paused_key, &paused);
    Ok(())
}
