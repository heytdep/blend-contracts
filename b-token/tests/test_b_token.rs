use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    vec, Address, BytesN, Env, IntoVal, Status, Symbol,
};

mod common;
use crate::common::{create_b_token, create_lending_pool, DTokenClient, TokenError};

fn create_and_init_b_token(
    e: &Env,
    pool: &Address,
    pool_id: &BytesN<32>,
    asset: &BytesN<32>,
    index: &u32,
) -> (BytesN<32>, DTokenClient) {
    let (b_token_id, b_token_client) = create_b_token(e);
    b_token_client.initialize(pool, &7, &"name".into_val(e), &"symbol".into_val(e));
    b_token_client.init_asset(pool, pool_id, &asset, index);
    (b_token_id, b_token_client)
}

#[test]
fn test_mint() {
    let e = Env::default();

    let bombadil = Address::random(&e);
    let underlying = e.register_stellar_asset_contract(bombadil);

    let pool_id = BytesN::<32>::random(&e);
    let pool = Address::from_contract_id(&e, &pool_id);
    let (b_token_id, b_token_client) =
        create_and_init_b_token(&e, &pool, &pool_id, &underlying, &2);

    let samwise = Address::random(&e);
    let sauron = Address::random(&e);

    // verify happy path
    b_token_client.mint(&pool, &samwise, &123456789);
    let authorizations = e.recorded_top_authorizations();
    assert_eq!(
        authorizations[0],
        (
            pool.clone(),
            b_token_id.clone(),
            Symbol::new(&e, "mint"),
            vec![
                &e,
                pool.clone().to_raw(),
                samwise.clone().to_raw(),
                123456789_i128.into_val(&e)
            ]
        )
    );
    assert_eq!(123456789, b_token_client.balance(&samwise));

    // verify only pool can mint
    let result = b_token_client.try_mint(&sauron, &samwise, &2);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::UnauthorizedError)
    );

    // verify can't mint a negative number
    let result = b_token_client.try_mint(&pool, &samwise, &-1);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::NegativeAmountError)
    );
}

#[test]
fn test_clawback() {
    let e = Env::default();

    let bombadil = Address::random(&e);
    let underlying = e.register_stellar_asset_contract(bombadil);

    let pool_id = BytesN::<32>::random(&e);
    let pool = Address::from_contract_id(&e, &pool_id);
    let (b_token_id, b_token_client) =
        create_and_init_b_token(&e, &pool, &pool_id, &underlying, &2);

    let samwise = Address::random(&e);
    let sauron = Address::random(&e);

    // verify happy path
    b_token_client.mint(&pool, &samwise, &123456789);
    assert_eq!(123456789, b_token_client.balance(&samwise));

    b_token_client.clawback(&pool, &samwise, &23456789);
    let authorizations = e.recorded_top_authorizations();
    assert_eq!(
        authorizations[0],
        (
            pool.clone(),
            b_token_id.clone(),
            Symbol::new(&e, "clawback"),
            vec![
                &e,
                pool.clone().to_raw(),
                samwise.clone().to_raw(),
                23456789_i128.into_val(&e)
            ]
        )
    );
    assert_eq!(100000000, b_token_client.balance(&samwise));

    // verify only pool can clawback
    let result = b_token_client.try_clawback(&sauron, &samwise, &2);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::UnauthorizedError)
    );

    // verify can't clawback a negative number
    let result = b_token_client.try_clawback(&pool, &samwise, &-1);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::NegativeAmountError)
    );
}

#[test]
fn test_incr_allow() {
    let e = Env::default();

    let bombadil = Address::random(&e);
    let underlying = e.register_stellar_asset_contract(bombadil);

    let res_index = 7;
    let pool_id = BytesN::<32>::random(&e);
    let pool = Address::from_contract_id(&e, &pool_id);
    let (b_token_id, b_token_client) =
        create_and_init_b_token(&e, &pool, &pool_id, &underlying, &res_index);

    let samwise = Address::random(&e);
    let spender = Address::random(&e);

    // verify happy path
    b_token_client.incr_allow(&samwise, &spender, &123456789);
    let authorizations = e.recorded_top_authorizations();
    assert_eq!(
        authorizations[0],
        (
            samwise.clone(),
            b_token_id.clone(),
            Symbol::new(&e, "incr_allow"),
            vec![
                &e,
                samwise.clone().to_raw(),
                spender.clone().to_raw(),
                123456789_i128.into_val(&e)
            ]
        )
    );
    assert_eq!(123456789, b_token_client.allowance(&samwise, &spender));

    // verify negative balance cannot be used
    let result = b_token_client.try_incr_allow(&samwise, &spender, &-1);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::NegativeAmountError)
    );
}

#[test]
fn test_decr_allow() {
    let e = Env::default();

    let bombadil = Address::random(&e);
    let underlying = e.register_stellar_asset_contract(bombadil);

    let res_index = 7;
    let pool_id = BytesN::<32>::random(&e);
    let pool = Address::from_contract_id(&e, &pool_id);
    let (b_token_id, b_token_client) =
        create_and_init_b_token(&e, &pool, &pool_id, &underlying, &res_index);

    let samwise = Address::random(&e);
    let spender = Address::random(&e);

    // verify happy path
    b_token_client.incr_allow(&samwise, &spender, &123456789);
    b_token_client.decr_allow(&samwise, &spender, &23456789);
    let authorizations = e.recorded_top_authorizations();
    assert_eq!(
        authorizations[0],
        (
            samwise.clone(),
            b_token_id.clone(),
            Symbol::new(&e, "decr_allow"),
            vec![
                &e,
                samwise.clone().to_raw(),
                spender.clone().to_raw(),
                23456789_i128.into_val(&e)
            ]
        )
    );
    assert_eq!(100000000, b_token_client.allowance(&samwise, &spender));

    // verify negative balance cannot be used
    let result = b_token_client.try_decr_allow(&samwise, &spender, &-1);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::NegativeAmountError)
    );
}

#[test]
fn test_xfer() {
    let e = Env::default();

    let bombadil = Address::random(&e);
    let underlying = e.register_stellar_asset_contract(bombadil);

    let res_index = 7;
    let (pool_id, pool_client) = create_lending_pool(&e);
    let pool = Address::from_contract_id(&e, &pool_id);
    let (b_token_id, b_token_client) =
        create_and_init_b_token(&e, &pool, &pool_id, &underlying, &res_index);

    let samwise = Address::random(&e);
    let frodo = Address::random(&e);

    // verify happy path
    b_token_client.mint(&pool, &samwise, &123456789);
    assert_eq!(123456789, b_token_client.balance(&samwise));
    pool_client.set_collat(&samwise, &res_index, &false);

    b_token_client.xfer(&samwise, &frodo, &23456789);
    let authorizations = e.recorded_top_authorizations();
    assert_eq!(
        authorizations[0],
        (
            samwise.clone(),
            b_token_id.clone(),
            Symbol::new(&e, "xfer"),
            vec![
                &e,
                samwise.clone().to_raw(),
                frodo.clone().to_raw(),
                23456789_i128.into_val(&e)
            ]
        )
    );
    assert_eq!(100000000, b_token_client.balance(&samwise));
    assert_eq!(23456789, b_token_client.balance(&frodo));

    // verify collateralized balance cannot transfer
    pool_client.set_collat(&frodo, &res_index, &true);
    let result = b_token_client.try_xfer(&frodo, &samwise, &1);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::UnauthorizedError)
    );

    // verify negative balance cannot be used
    let result = b_token_client.try_xfer(&samwise, &frodo, &-1);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::NegativeAmountError)
    );
}

#[test]
fn test_xfer_from() {
    let e = Env::default();

    let bombadil = Address::random(&e);
    let underlying = e.register_stellar_asset_contract(bombadil);

    let res_index = 7;
    let (pool_id, pool_client) = create_lending_pool(&e);
    let pool = Address::from_contract_id(&e, &pool_id);
    let (b_token_id, b_token_client) =
        create_and_init_b_token(&e, &pool, &pool_id, &underlying, &res_index);

    let samwise = Address::random(&e);
    let frodo = Address::random(&e);
    let spender = Address::random(&e);

    // verify happy path
    b_token_client.mint(&pool, &samwise, &123456789);
    assert_eq!(123456789, b_token_client.balance(&samwise));
    pool_client.set_collat(&samwise, &res_index, &false);

    b_token_client.incr_allow(&samwise, &spender, &223456789);
    b_token_client.xfer_from(&spender, &samwise, &frodo, &23456789);
    let authorizations = e.recorded_top_authorizations();
    assert_eq!(
        authorizations[0],
        (
            spender.clone(),
            b_token_id.clone(),
            Symbol::new(&e, "xfer_from"),
            vec![
                &e,
                spender.clone().to_raw(),
                samwise.clone().to_raw(),
                frodo.clone().to_raw(),
                23456789_i128.into_val(&e)
            ]
        )
    );
    assert_eq!(100000000, b_token_client.balance(&samwise));
    assert_eq!(23456789, b_token_client.balance(&frodo));
    assert_eq!(200000000, b_token_client.allowance(&samwise, &spender));

    // verify negative balance cannot be used
    let result = b_token_client.try_xfer_from(&spender, &samwise, &frodo, &-1);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::NegativeAmountError)
    );

    // verify collateralized balance cannot transfer
    pool_client.set_collat(&samwise, &res_index, &true);
    let result = b_token_client.try_xfer_from(&spender, &samwise, &frodo, &1);
    assert_eq!(
        result.unwrap_err().unwrap(),
        Status::from(TokenError::UnauthorizedError)
    );
}
