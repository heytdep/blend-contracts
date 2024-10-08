use soroban_fixed_point_math::FixedPoint;
use soroban_sdk::{unwrap::UnwrapOptimized, Env};

use crate::{constants::SCALAR_7, storage};

use super::{pool::Pool, Positions};

pub struct PositionData {
    /// The effective collateral balance denominated in the base asset
    pub collateral_base: i128,
    // The raw collateral balance demoninated in the base asset
    pub collateral_raw: i128,
    /// The effective liability balance denominated in the base asset
    pub liability_base: i128,
    // The raw liability balance demoninated in the base asset
    pub liability_raw: i128,
    /// The scalar for the base asset
    pub scalar: i128,
}

impl PositionData {
    /// Calculate the position data for a given set of of positions
    ///
    /// ### Arguments
    /// * pool - The pool
    /// * positions - The positions to calculate the health factor for
    pub fn calculate_from_positions(e: &Env, pool: &mut Pool, positions: &Positions) -> Self {
        let oracle_scalar = 10i128.pow(pool.load_price_decimals(e));

        let reserve_list = storage::get_res_list(e);

        let mut collateral_base = 0;
        let mut liability_base = 0;
        let mut collateral_raw = 0;
        let mut liability_raw = 0;
        for i in 0..reserve_list.len() {
            let b_token_balance = positions.collateral.get(i).unwrap_or(0);
            let d_token_balance = positions.liabilities.get(i).unwrap_or(0);
            if b_token_balance == 0 && d_token_balance == 0 {
                continue;
            }
            let reserve = pool.load_reserve(e, &reserve_list.get_unchecked(i), false);
            let asset_to_base = 1;

            if b_token_balance > 0 {
                // append users effective collateral to collateral_base
                let asset_collateral = reserve.to_effective_asset_from_b_token(b_token_balance);
                collateral_base += asset_to_base
                    .fixed_mul_floor(asset_collateral, reserve.scalar)
                    .unwrap_optimized();
                collateral_raw += asset_to_base
                    .fixed_mul_floor(
                        reserve.to_asset_from_b_token(b_token_balance),
                        reserve.scalar,
                    )
                    .unwrap_optimized();
            }

            if d_token_balance > 0 {
                // append users effective liability to liability_base
                let asset_liability = reserve.to_effective_asset_from_d_token(d_token_balance);
                e.events().publish(
                    (
                        "asset_liability",
                        asset_to_base,
                        asset_liability,
                        reserve.scalar,
                    ),
                    (),
                );
                liability_base += asset_to_base
                    .fixed_mul_ceil(asset_liability, reserve.scalar)
                    .unwrap_optimized();

                liability_raw += asset_to_base
                    .fixed_mul_ceil(
                        reserve.to_asset_from_d_token(d_token_balance),
                        reserve.scalar,
                    )
                    .unwrap_optimized();
            }
            pool.cache_reserve(reserve);
        }

        PositionData {
            collateral_base,
            collateral_raw,
            liability_base,
            liability_raw,
            scalar: oracle_scalar,
        }
    }

    /// Return the health factor as a ratio
    pub fn as_health_factor(&self) -> i128 {
        self.collateral_base
            .fixed_div_floor(self.liability_base, self.scalar)
            .unwrap_or(0)
    }

    // Check if the position data is over a maximum health factor
    // Note: max must be 7 decimals
    pub fn is_hf_over(&self, max: i128) -> bool {
        if self.liability_base == 0 {
            return true;
        }
        let min_health_factor = self.scalar.fixed_mul_ceil(max, SCALAR_7).unwrap_optimized();
        if self.as_health_factor() > min_health_factor {
            return true;
        }
        false
    }

    /// Check if the position data is under a minimum health factor
    /// Note: min must be 7 decimals
    pub fn is_hf_under(&self, min: i128) -> bool {
        if self.liability_base == 0 {
            return false;
        }
        let min_health_factor = self
            .scalar
            .fixed_mul_floor(min, SCALAR_7)
            .unwrap_optimized();
        if self.as_health_factor() < min_health_factor {
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{storage::PoolConfig, testutils};
    use sep_40_oracle::testutils::Asset;
    use soroban_sdk::{
        map,
        testutils::{Address as _, Ledger, LedgerInfo},
        vec, Address, Symbol,
    };

    #[test]
    fn test_calculate_from_positions() {
        let e = Env::default();
        e.budget().reset_unlimited();
        e.mock_all_auths();

        let bombadil = Address::generate(&e);
        let pool = testutils::create_pool(&e);
        let (oracle, oracle_client) = testutils::create_mock_oracle(&e);

        let (underlying_0, _) = testutils::create_token_contract(&e, &bombadil);
        let (reserve_config, reserve_data) = testutils::default_reserve_meta();
        testutils::create_reserve(&e, &pool, &underlying_0, &reserve_config, &reserve_data);

        let (underlying_1, _) = testutils::create_token_contract(&e, &bombadil);
        let (mut reserve_config, mut reserve_data) = testutils::default_reserve_meta();
        reserve_config.decimals = 9;
        reserve_config.c_factor = 0_8500000;
        reserve_config.l_factor = 0_8000000;
        reserve_data.b_supply = 100_000_000_000;
        reserve_data.d_supply = 70_000_000_000;
        reserve_data.b_rate = 1_100_000_000;
        reserve_data.d_rate = 1_150_000_000;
        reserve_config.index = 1;
        testutils::create_reserve(&e, &pool, &underlying_1, &reserve_config, &reserve_data);

        let (underlying_2, _) = testutils::create_token_contract(&e, &bombadil);
        let (mut reserve_config, mut reserve_data) = testutils::default_reserve_meta();
        reserve_config.decimals = 6;
        reserve_config.index = 2;
        reserve_data.b_supply = 10_000_000;
        reserve_data.d_supply = 5_000_000;
        reserve_data.b_rate = 1_001_100_000;
        reserve_data.d_rate = 1_001_200_000;
        testutils::create_reserve(&e, &pool, &underlying_2, &reserve_config, &reserve_data);

        oracle_client.set_data(
            &bombadil,
            &Asset::Other(Symbol::new(&e, "USD")),
            &vec![
                &e,
                Asset::Stellar(underlying_0),
                Asset::Stellar(underlying_1),
                Asset::Stellar(underlying_2),
            ],
            &7,
            &300,
        );
        oracle_client.set_price_stable(&vec![&e, 1_0000000, 2_5000000, 1000_0000000]);

        e.ledger().set(LedgerInfo {
            timestamp: 0,
            protocol_version: 20,
            sequence_number: 1234,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 3110400,
        });
        let pool_config = PoolConfig {
            oracle,
            bstop_rate: 0_2000000,
            status: 0,
            max_positions: 5,
        };

        let positions = Positions {
            liabilities: map![&e, (0, 1_5000000), (1, 50_987_654_321)],
            collateral: map![&e, (0, 100_1234567), (2, 0_250_000)],
            supply: map![&e, (1, 120_987_654_321)],
        };
        e.as_contract(&pool, || {
            storage::set_pool_config(&e, &pool_config);
            let mut pool = Pool::load(&e);
            let position_data = PositionData::calculate_from_positions(&e, &mut pool, &positions);
            assert_eq!(position_data.collateral_base, 262_7985925);
            assert_eq!(position_data.liability_base, 185_2368828);
            assert_eq!(position_data.collateral_raw, 350_3984567);
            assert_eq!(position_data.liability_raw, 148_0895062);
            assert_eq!(position_data.scalar, SCALAR_7);
        });
    }

    #[test]
    fn test_as_health_factor_rounds_floor() {
        let position_data = PositionData {
            collateral_base: 9_1234567,
            collateral_raw: 0,
            liability_base: 9_1000000,
            liability_raw: 0,
            scalar: 1_0000000,
        };

        // actual: 1.002577659
        let result = position_data.as_health_factor();
        assert_eq!(result, 1_0025776);
    }

    #[test]
    fn test_is_hf_under() {
        let position_data = PositionData {
            collateral_base: 9_1234567,
            collateral_raw: 12_0000000,
            liability_base: 9_1233333,
            liability_raw: 10_0000000,
            scalar: 1_0000000,
        };

        let result = position_data.is_hf_under(1_0000100);
        // no panic
        assert_eq!(result, false);
    }

    #[test]
    fn test_is_hf_under_odd_scalar() {
        let position_data = PositionData {
            collateral_base: 9_12345,
            collateral_raw: 12_00000,
            liability_base: 9_12333,
            liability_raw: 10_00000,
            scalar: 1_00000,
        };

        let result = position_data.is_hf_under(1_0000100);
        // no panic
        assert_eq!(result, false);
    }

    #[test]
    fn test_is_hf_under_no_liabilites() {
        let position_data = PositionData {
            collateral_base: 9_1234567,
            collateral_raw: 12_0000000,
            liability_base: 0,
            liability_raw: 0,
            scalar: 1_0000000,
        };

        let result = position_data.is_hf_under(1_0000100);
        // no panic
        assert_eq!(result, false);
    }

    #[test]
    fn test_is_hf_under_true() {
        let position_data = PositionData {
            collateral_base: 9_1234567,
            collateral_raw: 12_0000000,
            liability_base: 9_1234567,
            liability_raw: 10_0000000,
            scalar: 1_0000000,
        };

        let result = position_data.is_hf_under(1_0000100);
        // panic
        assert!(result);
    }

    #[test]
    fn test_is_hf_over() {
        let position_data = PositionData {
            collateral_base: 9_1234567,
            collateral_raw: 12_0000000,
            liability_base: 9_1233333,
            liability_raw: 10_0000000,
            scalar: 1_0000000,
        };

        let result = position_data.is_hf_over(1_1000000);
        // no panic
        assert_eq!(result, false);
    }

    #[test]
    fn test_is_hf_over_odd_scalar() {
        let position_data = PositionData {
            collateral_base: 9_1234567_000,
            collateral_raw: 12_0000000_000,
            liability_base: 9_1233333_000,
            liability_raw: 10_0000000_000,
            scalar: 1_0000000_000,
        };

        let result = position_data.is_hf_over(1_1000000);
        // no panic
        assert_eq!(result, false);
    }

    #[test]
    fn test_is_hf_over_no_liabilites() {
        let position_data = PositionData {
            collateral_base: 9_1234567,
            collateral_raw: 12_0000000,
            liability_base: 0,
            liability_raw: 0,
            scalar: 1_0000000,
        };

        let result = position_data.is_hf_over(1_0000100);
        // panic
        assert!(result);
    }
    #[test]
    fn test_is_hf_over_true() {
        let position_data = PositionData {
            collateral_base: 19_1234567,
            collateral_raw: 22_0000000,
            liability_base: 9_1234567,
            liability_raw: 10_0000000,
            scalar: 1_0000000,
        };

        let result = position_data.is_hf_over(1_0000100);
        // panic
        assert!(result);
    }
}
