#![no_std]
#[cfg(any(test, feature = "testutils"))]
extern crate std;

#[cfg(any(test, feature = "testutils"))]
pub use pool::{Pool as PoolState, PositionData, Reserve};

mod auctions;
mod constants;
mod contract;
mod dependencies;
mod emissions;
mod errors;
mod pool;
mod storage;
mod testutils;
mod validator;

pub use auctions::{AuctionData, AuctionType};
pub use contract::*;
pub use emissions::ReserveEmissionMetadata;
pub use errors::PoolError;
pub use pool::{Positions, Request, RequestType};
pub use storage::{
    AuctionKey, PoolConfig, PoolDataKey, PoolEmissionConfig, ReserveConfig, ReserveData,
    ReserveEmissionsConfig, ReserveEmissionsData, UserEmissionData, UserReserveKey,
};

const REFLECTOR_ORACLE_OFFCHAIN_PRICES: &'static str = env!("REFLECTOR_ORACLE_OFFCHAIN_PRICES");
const REFLECTOR_ORACLE_PUBNET_PRICES: &'static str = env!("REFLECTOR_ORACLE_PUBNET_PRICES");

mod reflector_oracle {
    use soroban_fixed_point_math::SorobanFixedPoint;
    use soroban_sdk::{
        contracttype, symbol_short, token::TokenClient, Address, Env, IntoVal, String, Symbol, Val,
    };

    use crate::{REFLECTOR_ORACLE_OFFCHAIN_PRICES, REFLECTOR_ORACLE_PUBNET_PRICES};

    #[contracttype]
    pub struct PriceData {
        price: i128,    //asset price at given point in time
        timestamp: u64, //recording timestamp
    }

    #[contracttype]
    enum Asset {
        Stellar(Address), //for Stellar Classic and Soroban assets
        Other(Symbol),    //for any external tokens/assets/symbols
    }

    pub fn get_token_amount_in_usdc_value(
        e: &Env,
        oracle_address: &Address,
        token_client: &TokenClient,
        amount: i128,
    ) -> i128 {
        let token_symbol = token_client.symbol();

        let asset = if token_symbol == String::from_str(&e, "BTC") {
            Some(Asset::Other(symbol_short!("BTC")))
        } else if token_symbol == String::from_str(&e, "ETH") {
            Some(Asset::Other(symbol_short!("ETH")))
        } else if token_symbol == String::from_str(&e, "USDT") {
            Some(Asset::Other(symbol_short!("USDT")))
        } else if token_symbol == String::from_str(&e, "XRP") {
            Some(Asset::Other(symbol_short!("XRP")))
        } else if token_symbol == String::from_str(&e, "SOL") {
            Some(Asset::Other(symbol_short!("SOL")))
        } else if token_symbol == String::from_str(&e, "USDC") {
            Some(Asset::Other(symbol_short!("USDC")))
        } else if token_symbol == String::from_str(&e, "ADA") {
            Some(Asset::Other(symbol_short!("ADA")))
        } else if token_symbol == String::from_str(&e, "AVAX") {
            Some(Asset::Other(symbol_short!("AVAX")))
        } else if token_symbol == String::from_str(&e, "DOT") {
            Some(Asset::Other(symbol_short!("DOT")))
        } else if token_symbol == String::from_str(&e, "MATIC") {
            Some(Asset::Other(symbol_short!("MATIC")))
        } else if token_symbol == String::from_str(&e, "LINK") {
            Some(Asset::Other(symbol_short!("LINK")))
        } else if token_symbol == String::from_str(&e, "DAI") {
            Some(Asset::Other(symbol_short!("DAI")))
        } else if token_symbol == String::from_str(&e, "ATOM") {
            Some(Asset::Other(symbol_short!("ATOM")))
        } else if token_symbol == String::from_str(&e, "native") {
            Some(Asset::Other(symbol_short!("XLM")))
        } else if token_symbol == String::from_str(&e, "UNI") {
            Some(Asset::Other(symbol_short!("UNI")))
        } else if token_symbol == String::from_str(&e, "EURC") {
            Some(Asset::Other(symbol_short!("EURC")))
        } else {
            None
        };

        /*let result = if let Some(asset) = asset {
            /*let last_timestamp = e.try_invoke_contract::<u64, Val>(
                &Address::from_string(&String::from_str(&e, REFLECTOR_ORACLE_OFFCHAIN_PRICES)),
                &Symbol::new(&e, "last_timestamp"),
                ().into_val(e),
            );*/

            e.try_invoke_contract::<Option<PriceData>, Val>(
                &Address::from_string(&String::from_str(&e, REFLECTOR_ORACLE_OFFCHAIN_PRICES)),
                &symbol_short!("lastprice"),
                (asset /*last_timestamp.unwrap().unwrap()*/,).into_val(e),
            )
        } else {
            /*let last_timestamp = e.try_invoke_contract::<u64, Val>(
                            &Address::from_string(&String::from_str(&e, REFLECTOR_ORACLE_PUBNET_PRICES)),
                            &Symbol::new(&e, "last_timestamp"),
                            ().into_val(e),
                        );
            */
            e.try_invoke_contract::<Option<PriceData>, Val>(
                &Address::from_string(&String::from_str(&e, REFLECTOR_ORACLE_PUBNET_PRICES)),
                &symbol_short!("lastprice"),
                (
                    Asset::Stellar(token_client.address.clone()),
                    /*last_timestamp.unwrap().unwrap(),*/
                )
                    .into_val(e),
            )
        };*/

        let result = if let Some(asset) = asset {
            let last_timestamp = e.try_invoke_contract::<u64, Val>(
                &Address::from_string(&String::from_str(&e, REFLECTOR_ORACLE_OFFCHAIN_PRICES)),
                &Symbol::new(&e, "last_timestamp"),
                ().into_val(e),
            );

            e.try_invoke_contract::<Option<PriceData>, Val>(
                &Address::from_string(&String::from_str(&e, REFLECTOR_ORACLE_OFFCHAIN_PRICES)),
                &symbol_short!("lastprice"),
                (asset, ).into_val(e),
            )
        } else {
            let last_timestamp = e.try_invoke_contract::<u64, Val>(
                &Address::from_string(&String::from_str(&e, REFLECTOR_ORACLE_PUBNET_PRICES)),
                &Symbol::new(&e, "last_timestamp"),
                ().into_val(e),
            );

            e.try_invoke_contract::<Option<PriceData>, Val>(
                &Address::from_string(&String::from_str(&e, REFLECTOR_ORACLE_PUBNET_PRICES)),
                &symbol_short!("lastprice"),
                (
                    Asset::Stellar(token_client.address.clone()),
                    //last_timestamp.unwrap().unwrap(),
                )
                    .into_val(e),
            )
        };

        let in_amount_usd_stellar_oracle = result
            .unwrap()
            .unwrap()
            .unwrap_or(PriceData {
                price: 0,
                timestamp: 0,
            })
            .price;

        let in_amount_usd_stellar_oracle =
            (in_amount_usd_stellar_oracle as i128).fixed_mul_ceil(&e, amount, 100000000000000);

        in_amount_usd_stellar_oracle
    }
}
