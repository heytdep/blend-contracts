#![cfg(test)]
use cast::i128;
use common::generate_contract_id;
use soroban_auth::{Identifier, Signature};
use soroban_sdk::{
    testutils::{Accounts, Ledger, LedgerInfo},
    BytesN, Env,
};

mod common;
use crate::common::{
    create_backstop_module, create_mock_pool_factory, create_token_from_id, BackstopError,
};

#[test]
fn test_draw_happy_path() {
    let e = Env::default();

    let (backstop_addr, backstop_client) = create_backstop_module(&e);
    let backstop_id = Identifier::Contract(backstop_addr.clone());

    let bombadil = e.accounts().generate_and_create();
    let bombadil_id = Identifier::Account(bombadil.clone());

    let token_id = BytesN::from_array(&e, &[222; 32]);
    let token_client = create_token_from_id(&e, &token_id, &bombadil_id);

    let pool_1 = generate_contract_id(&e);
    let pool_2 = generate_contract_id(&e);

    let mock_pool_factory = create_mock_pool_factory(&e);
    mock_pool_factory.set_pool(&pool_1);
    mock_pool_factory.set_pool(&pool_2);

    let samwise = e.accounts().generate_and_create();
    let samwise_id = Identifier::Account(samwise.clone());

    token_client.with_source_account(&bombadil).mint(
        &Signature::Invoker,
        &0,
        &samwise_id,
        &600_000_0000000,
    );
    token_client.with_source_account(&samwise).incr_allow(
        &Signature::Invoker,
        &0,
        &backstop_id,
        &i128(u64::MAX),
    );

    e.ledger().set(LedgerInfo {
        timestamp: 1441065600,
        protocol_version: 1,
        sequence_number: 0,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    backstop_client
        .with_source_account(&samwise)
        .deposit(&pool_1, &400_000_0000000);
    backstop_client
        .with_source_account(&samwise)
        .deposit(&pool_2, &200_000_0000000);

    let (pre_tokens_1, pre_shares_1, _pre_q4w_1) = backstop_client.p_balance(&pool_1);
    let (pre_tokens_2, pre_shares_2, _pre_q4w_2) = backstop_client.p_balance(&pool_2);

    assert_eq!(pre_tokens_1, 400_000_0000000);
    assert_eq!(pre_shares_1, 400_000_0000000);
    assert_eq!(pre_tokens_2, 200_000_0000000);
    assert_eq!(pre_shares_2, 200_000_0000000);

    // draw
    e.as_contract(&pool_2, || {
        backstop_client.draw(&100_000_0000000, &samwise_id);
    });

    let (post_tokens_1, post_shares_1, _post_q4w_1) = backstop_client.p_balance(&pool_1);
    let (post_tokens_2, post_shares_2, _post_q4w_2) = backstop_client.p_balance(&pool_2);

    assert_eq!(post_tokens_1, 400_000_0000000);
    assert_eq!(post_shares_1, 400_000_0000000);
    assert_eq!(post_tokens_2, 200_000_0000000 - 100_000_0000000);
    assert_eq!(post_shares_2, 200_000_0000000);
    assert_eq!(token_client.balance(&samwise_id), 100_000_0000000);
    assert_eq!(token_client.balance(&backstop_id), 500_000_0000000);
}

#[test]
fn test_draw_not_pool() {
    let e = Env::default();

    let (backstop_addr, backstop_client) = create_backstop_module(&e);
    let backstop_id = Identifier::Contract(backstop_addr.clone());

    let bombadil = e.accounts().generate_and_create();
    let bombadil_id = Identifier::Account(bombadil.clone());

    let token_id = BytesN::from_array(&e, &[222; 32]);
    let token_client = create_token_from_id(&e, &token_id, &bombadil_id);

    let pool_1 = generate_contract_id(&e);
    let pool_2 = generate_contract_id(&e);

    let mock_pool_factory = create_mock_pool_factory(&e);
    mock_pool_factory.set_pool(&pool_1);
    mock_pool_factory.set_pool(&pool_2);

    let samwise = e.accounts().generate_and_create();
    let samwise_id = Identifier::Account(samwise.clone());

    let sauron = e.accounts().generate_and_create();

    token_client.with_source_account(&bombadil).mint(
        &Signature::Invoker,
        &0,
        &samwise_id,
        &600_000_0000000,
    );
    token_client.with_source_account(&samwise).incr_allow(
        &Signature::Invoker,
        &0,
        &backstop_id,
        &i128(u64::MAX),
    );

    e.ledger().set(LedgerInfo {
        timestamp: 1441065600,
        protocol_version: 1,
        sequence_number: 0,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    backstop_client
        .with_source_account(&samwise)
        .deposit(&pool_1, &400_000_0000000);
    backstop_client
        .with_source_account(&samwise)
        .deposit(&pool_2, &200_000_0000000);

    let (pre_tokens_1, pre_shares_1, _pre_q4w_1) = backstop_client.p_balance(&pool_1);
    let (pre_tokens_2, pre_shares_2, _pre_q4w_2) = backstop_client.p_balance(&pool_2);

    assert_eq!(pre_tokens_1, 400_000_0000000);
    assert_eq!(pre_shares_1, 400_000_0000000);
    assert_eq!(pre_tokens_2, 200_000_0000000);
    assert_eq!(pre_shares_2, 200_000_0000000);

    // draw
    let result = backstop_client
        .with_source_account(&sauron)
        .try_draw(&100_000_0000000, &samwise_id);

    match result {
        Ok(_) => {
            assert!(false);
        }
        Err(error) => match error {
            Ok(p_error) => assert_eq!(p_error, BackstopError::NotPool),
            Err(_) => assert!(false),
        },
    }
}
