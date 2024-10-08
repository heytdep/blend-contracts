# Blend Protocol *simple* Retroshades Data Feed.

This is a retroshades simple data feed for blend that acts as a plug-and-play solution for all Blend pools
to obtain data regarding the TVL, borrow and collateral actions. Even if this is the simple version, some highlights here are:
- USDC price with per-block accuracy.
- User historical tvl + liabilities and collateral.
- Pool historical tvl + liabilities and collateral.
- Access all transactions.
- Catchup-friendly. This retroshade program will work with catchups too, which for complex protocols might be tricky
to obtain.

## Instructions

0. Have a mainnet mercury account, or create one at https://main.mercurydata.app.
1. Clone the repo: `git clone https://github.com/heytdep/blend-contracts`.
2. Run `make` (Note that this will fail when the compilation for the /pool contract triggers).
3. `cd pool` and

```
DEPLOYMENT_SEQ="CURRENT_LEDGER_SEQUENCE_HERE" REFLECTOR_ORACLE_OFFCHAIN_PRICES="ORACLE_USED_FOR_NON_DEX_PRICES" REFLECTOR_ORACLE_PUBNET_PRICES="ORACLE_USED_FOR_DEX_PRICES" cargo build --release --target wasm32-unknown-unknown
```

Note that:
- `DEPLOYMENT_SEQ` is required only for catchups. You can just get the current ledger sequence and use it.
- `REFLECTOR_ORACLE_OFFCHAIN_PRICES` is the reflector oracle (or any compatible oracle) that fetches prices from external centralized/decentralized exchanges. Learn more at https://reflector.network/docs.
- `REFLECTOR_ORACLE_PUBNET_PRICES` is the reflector oracle (or any compatible oracle) that fetches prices from the stellar DEX. Learn more at https://reflector.network/docs.

In general, unless your pool as customized oracle settings the follwing command will work well in mainnet:

```
DEPLOYMENT_SEQ="SEQ" REFLECTOR_ORACLE_OFFCHAIN_PRICES="CAFJZQWSED6YAWZU3GWRTOCNPPCGBN32L7QV43XX5LZLFTK6JLN34DLN" REFLECTOR_ORACLE_PUBNET_PRICES="CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M" cargo build --release --target wasm32-unknown-unknown
```

4. Deploy the program to mercury (default name is `blend-pools-simple`):

```
mercury-cli --jwt $MERCURY_JWT --mainnet true retroshade --project "blend-pools-simple" --contracts "SOME_POOL" --contracts "SOME_OTHER_POOL" --target ../target/wasm32-unknown-unknown/release/pool.wasm
```

For instance, for indexing only the mainnet yieldblox pool:

```
mercury-cli --jwt $MERCURY_JWT --mainnet true retroshade --project "blend-pools-simple" --contracts "CBP7NO6F7FRDHSOFQBT2L2UWYIZ2PU76JKVRYAQTG3KZSQLYAOKIF2WB" --target ../target/wasm32-unknown-unknown/release/pool.wasm
```

5. (Optional) catchups. This will allow you to retrieve historical data in case the program is being deployed after the pools have been interacted with. Skip this step if your pool is not live yet.

```
mercury-cli --jwt $MERCURY_JWT --mainnet true catchup --retroshades true --project-name "blend-pools-simple" --functions "submit"
```

This will return the catchup id. You can fetch the status with `curl -X GET https://mainnet.mercurydata.app/catchups/CATCHUP_ID_HERE`. Retroshades catchups are extremely fast, and will allow you to ingest months of history in just a couple of minutes, depending on the amount of transaction your pool has had the catchup could be between near-instant finality or we've seen a record for the first blend pool at about 6 minutes.

6. Once the catchup is complete, you can query the program! Go to the mercury dashboards and select projects > "blend-pools-simple". This will open up a project dashboard where you can build the SQL queries to extract the data you required.

### Some query examples

> Note: remember to change the `borrow_action_info_TABLE_NAME` or `collateral_action_info_TABLE_NAME` to your table names.

1. Getting all current supply and borrowed amounts for each pool and each asset in both usdc and the asset denomination:

```sql
WITH ranked_data AS (
  SELECT
    pool,
    reserve_address,
    usdc_reserve_supply,
    usdc_reserve_liabilities,
    reserve_supply,
    reserve_liabilities,
    ledger,
    ROW_NUMBER() OVER (PARTITION BY pool, reserve_address ORDER BY ledger DESC) as rn
  FROM borrow_action_info_TABLE_NAME
)
SELECT
  pool,
  reserve_address,
  CAST(usdc_reserve_supply AS DECIMAL(38,7)) / POWER(10, 7) AS usdc_reserve_supply_adjusted,
  CAST(usdc_reserve_liabilities AS DECIMAL(38,7)) / POWER(10, 7) AS usdc_reserve_liabilities_adjusted,
  CAST(reserve_supply AS DECIMAL(38,7)) / POWER(10, 7) AS reserve_supply_adjusted,
  CAST(reserve_liabilities AS DECIMAL(38,7)) / POWER(10, 7) AS reserve_liabilities_adjusted,
  ledger AS latest_ledger
FROM ranked_data
WHERE rn = 1
ORDER BY pool, reserve_address;
```

2. Get all repaid amounts for each pool and asset

```sql
SELECT
  pool,
  reserve_address AS asset,
  SUM(CAST(amount AS DECIMAL(38,7)) / POWER(10, 7)) AS total_amount_repaid,
  SUM(CAST(usdc_amount AS DECIMAL(38,7)) / POWER(10, 7)) AS total_usdc_amount_repaid
FROM borrow_action_info_TABLE_NAME
WHERE action_type = 'repay'
GROUP BY pool, reserve_address
ORDER BY pool, reserve_address;
```

3. Total liabilities and supply (+ tvl) per pool counting all assets in usdc:

```sql
WITH asset_ledger_data AS (
    SELECT DISTINCT
        pool,
        reserve_address,
        ledger,
        usdc_reserve_liabilities,
        usdc_reserve_supply,
        ROW_NUMBER() OVER (PARTITION BY pool, reserve_address ORDER BY ledger) AS rn
    FROM borrow_action_info45f1680fc8840e87e355b7ca68bc4a4e
),

all_ledgers AS (
    SELECT DISTINCT pool, ledger
    FROM borrow_action_info45f1680fc8840e87e355b7ca68bc4a4e
),

asset_latest_values AS (
    SELECT
        al.pool,
        al.ledger,
        ald.reserve_address,
        ald.usdc_reserve_liabilities,
        ald.usdc_reserve_supply
    FROM all_ledgers al
    LEFT JOIN asset_ledger_data ald ON al.pool = ald.pool AND al.ledger >= ald.ledger
    LEFT JOIN asset_ledger_data next_ald
        ON ald.pool = next_ald.pool
        AND ald.reserve_address = next_ald.reserve_address
        AND ald.rn = next_ald.rn - 1
    WHERE next_ald.ledger IS NULL OR next_ald.ledger > al.ledger
),

pool_totals AS (
    SELECT
        pool,
        ledger,
        SUM(CAST(usdc_reserve_liabilities AS DECIMAL(38,7)) / POWER(10, 7)) AS total_liabilities,
        SUM(CAST(usdc_reserve_supply AS DECIMAL(38,7)) / POWER(10, 7)) AS total_supply
    FROM asset_latest_values
    GROUP BY pool, ledger
)

SELECT
    pool,
    ledger,
    total_liabilities,
    total_supply,
    total_liabilities + total_supply AS tvl
FROM pool_totals
ORDER BY pool, ledger desc;
```

> Tip: LLMs have gotten quite good at SQL. If you don't know how to write the queries for complex data (such as the above query) they can save you significant amounts of time.

<hr/>

# Blend Protocol

This repository contains the smart contacts for an implementation of the Blend Protocol. Blend is a universal liquidity protocol primitive that enables the permissionless creation of lending pools.

## Documentation

To learn more about the Blend Protocol, visit the docs:

- [Blend Docs](https://docs.blend.capital/)

## Audits

Conducted audits can be viewed in the `audits` folder.

## Getting Started

Build the contracts with:

```
make
```

Run all unit tests and the integration test suite with:

```
make test
```

## Deployment

The `make` command creates an optimized and un-optimized set of WASM contracts. It's recommended to use the optimized version if deploying to a network.

These can be found at the path:

```
target/wasm32-unknown-unknown/optimized
```

For help with deployment to a network, please visit the [Blend Utils](https://github.com/blend-capital/blend-utils) repo.

## Contributing

Notes for contributors:

- Under no circumstances should the "overflow-checks" flag be removed otherwise contract math will become unsafe

## Community Links

A set of links for various things in the community. Please submit a pull request if you would like a link included.

- [Blend Discord](https://discord.com/invite/a6CDBQQcjW)
