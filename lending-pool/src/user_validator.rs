use soroban_auth::Identifier;
use soroban_sdk::{Env, BigInt};

use crate::{user_data::{UserData, UserAction}};

/// Validate if a user is currently healthy given an incoming actions.
/// 
/// ### Arguments
/// * `user` - The user to check
/// * `user_action` - An incoming user action
pub fn validate_hf(e: &Env, user: &Identifier, user_action: &UserAction) -> bool {
    let account_data = UserData::load(e, user, &user_action);

    let collateral_required = (account_data.e_liability_base.clone() * BigInt::from_u64(e, 1_0500000)) / BigInt::from_u64(e, 1_0000000);
    return (collateral_required < account_data.e_collateral_base) || account_data.e_liability_base.is_zero();
}