use crate::{
    dependencies::TokenClient,
    errors::PoolError,
    reserve::Reserve,
    reserve_usage::ReserveUsage,
    storage,
    user_data::UserAction,
    validator::{require_hf, require_nonnegative},
};
use soroban_sdk::{Address, Env};

/// Perform a supply of "asset" from "from" for "amount" into the pool
///
/// Returns the number of b_tokens minted
pub fn execute_supply(
    e: &Env,
    from: &Address,
    asset: &Address,
    amount: i128,
) -> Result<i128, PoolError> {
    require_nonnegative(amount)?;
    let pool_config = storage::get_pool_config(e);

    if pool_config.status == 2 {
        return Err(PoolError::InvalidPoolStatus);
    }

    let mut reserve = Reserve::load(&e, asset.clone());
    reserve.pre_action(&e, &pool_config, 1, from.clone())?;

    let to_mint = reserve.to_b_token_down(e, amount.clone());
    if storage::has_auction(e, &0, &from) {
        let user_action = UserAction {
            asset: asset.clone(),
            b_token_delta: to_mint,
            d_token_delta: 0,
        };
        require_hf(&e, &pool_config, &from, &user_action)?;
        storage::del_auction(e, &0, &from);
    }

    TokenClient::new(&e, asset).transfer(from, &e.current_contract_address(), &amount);
    TokenClient::new(&e, &reserve.config.b_token).mint(&from, &to_mint);

    let mut user_config = ReserveUsage::new(storage::get_user_config(e, from));
    if !user_config.is_supply(reserve.config.index) {
        user_config.set_supply(reserve.config.index, true);
        storage::set_user_config(e, from, &user_config.config);
    }

    reserve.add_supply(&to_mint);
    reserve.set_data(&e);

    Ok(to_mint)
}

/// Perform an update of the collateral status to the
///
/// Returns the number of b_tokens minted
pub fn execute_update_collateral(
    e: &Env,
    user: &Address,
    asset: &Address,
    enable: bool,
) -> Result<(), PoolError> {
    let res_config = storage::get_res_config(e, asset);
    let mut user_config = ReserveUsage::new(storage::get_user_config(e, user));
    if !user_config.is_supply(res_config.index) {
        return Err(PoolError::BadRequest);
    }

    if user_config.is_collateral(res_config.index) && !enable {
        // user is disabling active collateral. Check their HF.

        // user_config is set first to remove the current asset from the HF calculation
        // without checking the b_token balance
        user_config.set_collateral_disabled(res_config.index, true);
        storage::set_user_config(e, user, &user_config.config);

        let pool_config = storage::get_pool_config(e);
        let user_action = UserAction {
            asset: asset.clone(),
            b_token_delta: 0,
            d_token_delta: 0,
        };
        require_hf(&e, &pool_config, user, &user_action)?;
    } else if !user_config.is_collateral(res_config.index) && enable {
        // user is enabled inactive collateral
        user_config.set_collateral_disabled(res_config.index, false);
        storage::set_user_config(e, user, &user_config.config);
    }
    // no change needed

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        auctions::AuctionData,
        dependencies::{TokenClient, B_TOKEN_WASM, D_TOKEN_WASM},
        pool::{execute_borrow, execute_initialize, initialize_reserve},
        storage::{PoolConfig, ReserveMetadata},
        testutils::{create_mock_oracle, create_reserve, create_token_contract, setup_reserve},
    };

    use super::*;
    use soroban_sdk::{
        map,
        testutils::{Address as _, Ledger, LedgerInfo},
        Symbol,
    };

    #[test]
    fn test_supply() {
        let e = Env::default();
        e.mock_all_auths();
        e.ledger().set(LedgerInfo {
            timestamp: 12345,
            protocol_version: 1,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 10,
        });
        let name: Symbol = Symbol::new(&e, "pool1");
        let pool_address = Address::random(&e);
        let backstop_address = Address::random(&e);
        let blnd_id = Address::random(&e);
        let usdc_id = Address::random(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);
        let frodo = Address::random(&e);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);

        let b_token_hash = e.install_contract_wasm(B_TOKEN_WASM);
        let d_token_hash = e.install_contract_wasm(D_TOKEN_WASM);
        e.as_contract(&pool_address, || {
            execute_initialize(
                &e,
                &bombadil,
                &name,
                &oracle_id,
                &0_200_000_000,
                &backstop_address,
                &b_token_hash,
                &d_token_hash,
                &blnd_id,
                &usdc_id,
            )
            .unwrap();
        });

        let metadata = ReserveMetadata {
            decimals: 7,
            c_factor: 0_7500000,
            l_factor: 0_7500000,
            util: 0_5000000,
            max_util: 0_9500000,
            r_one: 0_0500000,
            r_two: 0_5000000,
            r_three: 1_5000000,
            reactivity: 100,
        };
        let (asset_id_0, asset_0_client) = create_token_contract(&e, &bombadil);
        let (asset_id_1, asset_1_client) = create_token_contract(&e, &bombadil);
        e.as_contract(&pool_address, || {
            initialize_reserve(&e, &bombadil, &asset_id_0, &metadata).unwrap();
            initialize_reserve(&e, &bombadil, &asset_id_1, &metadata).unwrap();
        });

        oracle_client.set_price(&asset_id_0, &1_0000000);
        oracle_client.set_price(&asset_id_1, &1_0000000);
        asset_0_client.mint(&samwise, &500_0000000);
        asset_1_client.mint(&frodo, &500_0000000);

        e.ledger().set(LedgerInfo {
            timestamp: 14345,
            protocol_version: 1,
            sequence_number: 200,
            network_id: Default::default(),
            base_reserve: 10,
        });

        e.as_contract(&pool_address, || {
            e.budget().reset_unlimited();
            execute_supply(&e, &samwise, &asset_id_0, 100_0000000).unwrap();
            execute_supply(&e, &frodo, &asset_id_1, 500_0000000).unwrap();
            assert_eq!(400_0000000, asset_0_client.balance(&samwise));
            assert_eq!(0, asset_1_client.balance(&frodo));
            assert_eq!(100_0000000, asset_0_client.balance(&pool_address));
            assert_eq!(500_0000000, asset_1_client.balance(&pool_address));
            let user_config = ReserveUsage::new(storage::get_user_config(&e, &samwise));
            assert!(user_config.is_collateral(0));
        });
    }

    #[test]
    fn test_supply_rounds_b_tokens_down() {
        let e = Env::default();
        e.mock_all_auths();
        let pool_address = Address::random(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);
        let sauron = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        reserve_0.data.d_supply = 1_0000000;
        reserve_0.data.b_supply = 8_0000000;
        reserve_0.data.d_rate = 2_500000000;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_0);
        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &1_0000000);

        let asset_0_client = TokenClient::new(&e, &reserve_0.asset);
        asset_0_client.mint(&pool_address, &7_0000000); // supplied by samwise
        asset_0_client.mint(&samwise, &1_0000000); //borrowed by samwise
        asset_0_client.mint(&sauron, &2); //2 to be supplied by sauron
        let b_token0_client = TokenClient::new(&e, &reserve_0.config.b_token);
        b_token0_client.mint(&samwise, &8_0000000); //supplied by samwise
        let d_token0_client = TokenClient::new(&e, &reserve_0.config.d_token);
        d_token0_client.mint(&samwise, &1_0000000); //borrowed by samwise
        e.budget().reset_unlimited();

        let pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0,
            status: 0,
        };
        e.as_contract(&pool_address, || {
            storage::set_pool_config(&e, &pool_config);

            e.budget().reset_unlimited();

            // supply - unrounded
            let result = execute_supply(&e, &samwise, &reserve_0.asset, 1_0000000).unwrap();
            assert_eq!(result, 5333333);
            assert_eq!(0, asset_0_client.balance(&samwise));
            assert_eq!(result + 8_0000000, b_token0_client.balance(&samwise));

            // supply - rounded
            let result2 = execute_supply(&e, &sauron, &reserve_0.asset, 2).unwrap();
            assert_eq!(result2, 1);
            assert_eq!(0, asset_0_client.balance(&sauron));
            assert_eq!(1, b_token0_client.balance(&sauron));
        });
    }

    #[test]
    fn test_supply_user_being_liquidated() {
        let e = Env::default();
        e.mock_all_auths();
        let pool_address = Address::random(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);
        let frodo = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        reserve_0.data.d_supply = 0;
        reserve_0.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_0);

        let mut reserve_1 = create_reserve(&e);
        reserve_1.data.d_supply = 0;
        reserve_1.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_1);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &1_0000000);
        oracle_client.set_price(&reserve_1.asset, &1_0000000);

        let asset_0_client = TokenClient::new(&e, &reserve_0.asset);
        let asset_1_client = TokenClient::new(&e, &reserve_1.asset);
        asset_0_client.mint(&samwise, &500_0000000);
        asset_1_client.mint(&frodo, &500_0000000);

        let pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0,
            status: 0,
        };
        e.as_contract(&pool_address, || {
            storage::set_pool_config(&e, &pool_config);

            e.budget().reset_unlimited();
            execute_supply(&e, &frodo, &reserve_1.asset, 500_0000000).unwrap(); // for samwise to borrow
            execute_supply(&e, &samwise, &reserve_0.asset, 100_0000000).unwrap();
            execute_borrow(&e, &samwise, &reserve_1.asset, 50_0000000, &samwise).unwrap();
            assert_eq!(400_0000000, asset_0_client.balance(&samwise));
            assert_eq!(50_0000000, asset_1_client.balance(&samwise));
            assert_eq!(100_0000000, asset_0_client.balance(&pool_address));
            assert_eq!(450_0000000, asset_1_client.balance(&pool_address));

            // adjust prices to put samwise underwater
            oracle_client.set_price(&reserve_1.asset, &2_0000000);

            // mock a created liquidation auction
            storage::set_auction(
                &e,
                &0,
                &samwise,
                &AuctionData {
                    bid: map![&e],
                    lot: map![&e],
                    block: e.ledger().sequence(),
                },
            );

            let result = execute_supply(&e, &samwise, &reserve_0.asset, 50_0000000);
            assert_eq!(result, Err(PoolError::InvalidHf));

            execute_supply(&e, &samwise, &reserve_0.asset, 100_0000000).unwrap();
            assert_eq!(300_0000000, asset_0_client.balance(&samwise));
            assert_eq!(50_0000000, asset_1_client.balance(&samwise));
            assert_eq!(200_0000000, asset_0_client.balance(&pool_address));
            assert_eq!(450_0000000, asset_1_client.balance(&pool_address));
            assert_eq!(false, storage::has_auction(&e, &0, &samwise));
        });
    }

    #[test]
    fn test_supply_user_negative_amount() {
        let e = Env::default();
        e.mock_all_auths();
        let pool_address = Address::random(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);
        let frodo = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        reserve_0.data.d_supply = 0;
        reserve_0.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_0);

        let mut reserve_1 = create_reserve(&e);
        reserve_1.data.d_supply = 0;
        reserve_1.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_1);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &1_0000000);
        oracle_client.set_price(&reserve_1.asset, &1_0000000);

        let asset_0_client = TokenClient::new(&e, &reserve_0.asset);
        let asset_1_client = TokenClient::new(&e, &reserve_1.asset);
        asset_0_client.mint(&samwise, &500_0000000);
        asset_1_client.mint(&frodo, &500_0000000);

        let pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0,
            status: 0,
        };
        e.as_contract(&pool_address, || {
            storage::set_pool_config(&e, &pool_config);

            e.budget().reset_unlimited();
            execute_supply(&e, &frodo, &reserve_1.asset, 500_0000000).unwrap(); // for samwise to borrow
            execute_supply(&e, &samwise, &reserve_0.asset, 100_0000000).unwrap();
            execute_borrow(&e, &samwise, &reserve_1.asset, 50_0000000, &samwise).unwrap();
            assert_eq!(400_0000000, asset_0_client.balance(&samwise));
            assert_eq!(50_0000000, asset_1_client.balance(&samwise));
            assert_eq!(100_0000000, asset_0_client.balance(&pool_address));
            assert_eq!(450_0000000, asset_1_client.balance(&pool_address));

            // adjust prices to put samwise underwater
            oracle_client.set_price(&reserve_1.asset, &2_0000000);

            // mock a created liquidation auction
            storage::set_auction(
                &e,
                &0,
                &samwise,
                &AuctionData {
                    bid: map![&e],
                    lot: map![&e],
                    block: e.ledger().sequence(),
                },
            );

            let result = execute_supply(&e, &samwise, &reserve_0.asset, -50_0000000);
            assert_eq!(result, Err(PoolError::NegativeAmount));
        });
    }

    #[test]
    fn test_pool_supply_checks_status() {
        let e = Env::default();
        e.mock_all_auths();
        let pool_address = Address::random(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        reserve_0.data.d_supply = 0;
        reserve_0.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_0);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &1_0000000);

        let asset_0_client = TokenClient::new(&e, &reserve_0.asset);
        asset_0_client.mint(&samwise, &500_0000000);

        let mut pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0,
            status: 1,
        };
        e.as_contract(&pool_address, || {
            storage::set_pool_config(&e, &pool_config);
            e.budget().reset_unlimited();

            // can supply on ice
            execute_supply(&e, &samwise, &reserve_0.asset, 100_0000000).unwrap();
            assert_eq!(400_0000000, asset_0_client.balance(&samwise));
            assert_eq!(100_0000000, asset_0_client.balance(&pool_address));

            // can't supply if frozen
            pool_config.status = 2;
            storage::set_pool_config(&e, &pool_config);
            let result = execute_supply(&e, &samwise, &reserve_0.asset, 100_0000000);
            assert_eq!(result, Err(PoolError::InvalidPoolStatus));
        });
    }

    #[test]
    fn test_execute_update_reserve_not_active_collateral() {
        let e = Env::default();
        e.mock_all_auths();
        let pool_address = Address::random(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        reserve_0.data.d_supply = 0;
        reserve_0.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_0);

        let mut reserve_1 = create_reserve(&e);
        reserve_1.data.d_supply = 0;
        reserve_1.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_1);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &1_0000000);
        oracle_client.set_price(&reserve_1.asset, &1_0000000);

        let asset_0_client = TokenClient::new(&e, &reserve_0.asset);
        let asset_1_client = TokenClient::new(&e, &reserve_1.asset);
        asset_0_client.mint(&samwise, &500_0000000);
        asset_1_client.mint(&samwise, &500_0000000);

        let pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0,
            status: 0,
        };
        e.as_contract(&pool_address, || {
            storage::set_pool_config(&e, &pool_config);

            e.budget().reset_unlimited();
            execute_supply(&e, &samwise, &reserve_0.asset, 100_0000000).unwrap();
            execute_supply(&e, &samwise, &reserve_1.asset, 200_0000000).unwrap();

            // disable
            execute_update_collateral(&e, &samwise, &reserve_1.asset, false).unwrap();
            let new_user_config = ReserveUsage::new(storage::get_user_config(&e, &samwise));
            assert_eq!(new_user_config.is_collateral_disabled(1), true);
            assert_eq!(new_user_config.is_collateral(1), false);

            // enable
            execute_update_collateral(&e, &samwise, &reserve_1.asset, true).unwrap();
            let new_user_config = ReserveUsage::new(storage::get_user_config(&e, &samwise));
            assert_eq!(new_user_config.is_collateral_disabled(1), false);
            assert_eq!(new_user_config.is_collateral(1), true);
        });
    }

    #[test]
    fn test_execute_update_reserve_active_collateral() {
        let e = Env::default();
        e.mock_all_auths();
        let pool_address = Address::random(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);
        let frodo = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        reserve_0.data.d_supply = 0;
        reserve_0.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_0);

        let mut reserve_1 = create_reserve(&e);
        reserve_1.data.d_supply = 0;
        reserve_1.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_1);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &1_0000000);
        oracle_client.set_price(&reserve_1.asset, &1_0000000);

        let asset_0_client = TokenClient::new(&e, &reserve_0.asset);
        let asset_1_client = TokenClient::new(&e, &reserve_1.asset);
        asset_0_client.mint(&samwise, &500_0000000);
        asset_1_client.mint(&frodo, &500_0000000);
        asset_1_client.mint(&samwise, &500_0000000);

        let pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0,
            status: 0,
        };
        e.as_contract(&pool_address, || {
            storage::set_pool_config(&e, &pool_config);

            e.budget().reset_unlimited();
            execute_supply(&e, &frodo, &reserve_1.asset, 500_0000000).unwrap();
            execute_supply(&e, &samwise, &reserve_0.asset, 100_0000000).unwrap();
            execute_supply(&e, &samwise, &reserve_1.asset, 100_0000000).unwrap();
            execute_borrow(&e, &samwise, &reserve_1.asset, 50_0000000, &samwise).unwrap();

            // samwise can meet the collateral requirement with a single supply
            // disable
            execute_update_collateral(&e, &samwise, &reserve_0.asset, false).unwrap();
            let new_user_config = ReserveUsage::new(storage::get_user_config(&e, &samwise));
            assert_eq!(new_user_config.is_collateral_disabled(0), true);
            assert_eq!(new_user_config.is_collateral(0), false);

            // enable
            execute_update_collateral(&e, &samwise, &reserve_0.asset, true).unwrap();
            let new_user_config = ReserveUsage::new(storage::get_user_config(&e, &samwise));
            assert_eq!(new_user_config.is_collateral_disabled(0), false);
            assert_eq!(new_user_config.is_collateral(0), true);

            // borrow more tokens so a single supply is not sufficient collateral
            execute_borrow(&e, &samwise, &reserve_1.asset, 10_0000000, &samwise).unwrap();

            // disable
            let result = execute_update_collateral(&e, &samwise, &reserve_0.asset, false);
            assert_eq!(result, Err(PoolError::InvalidHf));
            let new_user_config = ReserveUsage::new(storage::get_user_config(&e, &samwise));
            assert_eq!(new_user_config.is_collateral_disabled(0), true);
            assert_eq!(new_user_config.is_collateral(0), false);
        });
    }

    #[test]
    fn test_execute_update_reserve_not_supplied() {
        let e = Env::default();
        e.mock_all_auths();
        let pool_address = Address::random(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        reserve_0.data.d_supply = 0;
        reserve_0.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_0);

        let mut reserve_1 = create_reserve(&e);
        reserve_1.data.d_supply = 0;
        reserve_1.data.b_supply = 0;
        setup_reserve(&e, &pool_address, &bombadil, &mut reserve_1);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &1_0000000);
        oracle_client.set_price(&reserve_1.asset, &1_0000000);

        let asset_0_client = TokenClient::new(&e, &reserve_0.asset);
        let asset_1_client = TokenClient::new(&e, &reserve_1.asset);
        asset_0_client.mint(&samwise, &500_0000000);
        asset_1_client.mint(&samwise, &500_0000000);

        let pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0,
            status: 0,
        };
        e.as_contract(&pool_address, || {
            storage::set_pool_config(&e, &pool_config);

            e.budget().reset_unlimited();
            let result = execute_update_collateral(&e, &samwise, &reserve_0.asset, false);
            assert_eq!(result, Err(PoolError::BadRequest));
        });
    }
}
