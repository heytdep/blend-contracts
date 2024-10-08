use crate::{
    backstop::{self, load_pool_backstop_data, PoolBackstopData, UserBalance, Q4W},
    dependencies::EmitterClient,
    emissions,
    errors::BackstopError,
    storage,
};
use soroban_sdk::{
    contract, contractclient, contractimpl, panic_with_error, Address, Env, Symbol, Vec,
};

pub(crate) mod retroshades {
    use retroshade_sdk::Retroshade;
    use soroban_sdk::{contracttype, Address, Symbol, Vec};

    #[contracttype]
    #[derive(Retroshade)]
    pub struct CollateralActionInfo {
        pub pool: Address,
        pub reserve_address: Address,
        pub user_address: Address,
        pub action_type: Symbol,
        pub amount: i128,
        pub b_tokens: i128,
        pub user_reserve_total_supply: i128,
        pub user_collateral_supply: i128,
        pub user_updated_emissions: i128,
        pub reserve_supply: i128,
        pub reserve_liabilities: i128,
        pub usdc_amount: i128,
        pub usdc_reserve_supply: i128,
        pub usdc_reserve_liabilities: i128,
        pub usdc_user_reserve_total_supply: i128,
        pub usdc_user_collateral_supply: i128,

        pub ledger: u32,
        pub timestamp: u64,
    }

    #[contracttype]
    #[derive(Retroshade)]
    pub struct BorrowActionInfo {
        pub pool: Address,
        pub reserve_address: Address,
        pub user_address: Address,
        pub action_type: Symbol,
        pub amount: i128,
        pub d_tokens: i128,
        pub utilization_rate: i128,

        pub user_reserve_total_supply: i128,
        pub user_liabilities: i128,
        pub user_updated_emissions: i128,

        pub reserve_supply: i128,
        pub reserve_liabilities: i128,
        pub usdc_amount: i128,
        pub usdc_reserve_supply: i128,
        pub usdc_reserve_liabilities: i128,
        pub usdc_user_reserve_total_supply: i128,
        pub usdc_user_liabilities: i128,

        pub ledger: u32,
        pub timestamp: u64,
    }

    #[contracttype]
    #[derive(Retroshade)]
    pub struct FillAuctionActionInfo {
        pub pool: Address,
        pub liquidator_address: Address,
        pub liquidatee_address: Address,

        pub auction_type: u32,
        pub percent_filled: i128,
        pub health: i128,

        pub backstop_balance_change: i128,
        pub asset_changes: Vec<Address>,
        pub amount_changes: Vec<i128>,

        pub to_fill_bid_assets: Vec<Address>,
        pub to_fill_bid_amounts: Vec<i128>,
        pub to_fill_bid_usdc_amounts: Vec<i128>,
        pub to_fill_lot_assets: Vec<Address>,
        pub to_fill_lot_amounts: Vec<i128>,
        pub to_fill_lot_usdc_amounts: Vec<i128>,
        pub remaining_bid_assets: Vec<Address>,
        pub remaining_bid_amounts: Vec<i128>,
        pub remaining_bid_usdc_amounts: Vec<i128>,
        pub remaining_lot_assets: Vec<Address>,
        pub remaining_lot_amounts: Vec<i128>,
        pub remaining_lot_usdc_amounts: Vec<i128>,

        pub ledger: u32,
        pub timestamp: u64,
    }

    #[contracttype]
    #[derive(Retroshade)]
    pub struct ClaimActionInfo {
        pub pool: Address,

        pub user: Address,
        pub to: Address,

        pub total_claimed: i128,
        pub claimed_reserves_idxs: Vec<u32>,
        pub claimed_user_balance: Vec<i128>,
        pub claimed_amounts: Vec<i128>,
        pub reserves_claimed_ratio: Vec<i128>,

        pub ledger: u32,
        pub timestamp: u64,
    }
}

/// ### Backstop
///
/// A backstop module for the Blend protocol's Isolated Lending Pools
#[contract]
pub struct BackstopContract;

#[contractclient(name = "BackstopClient")]
pub trait Backstop {
    /// Initialize the backstop
    ///
    /// This function requires that the Emitter has already been initialized
    ///
    /// ### Arguments
    /// * `backstop_token` - The backstop token ID - an LP token with the pair BLND:USDC
    /// * `emitter` - The Emitter contract ID
    /// * `blnd_token` - The BLND token ID
    /// * `usdc_token` - The USDC token ID
    /// * `pool_factory` - The pool factory ID
    /// * `drop_list` - The list of addresses to distribute initial BLND to and the percent of the distribution they should receive
    ///
    /// ### Errors
    /// If initialize has already been called
    fn initialize(
        e: Env,
        backstop_token: Address,
        emitter: Address,
        blnd_token: Address,
        usdc_token: Address,
        pool_factory: Address,
        drop_list: Vec<(Address, i128)>,
    );

    /********** Core **********/

    /// Deposit backstop tokens from "from" into the backstop of a pool
    ///
    /// Returns the number of backstop pool shares minted
    ///
    /// ### Arguments
    /// * `from` - The address depositing into the backstop
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of tokens to deposit
    fn deposit(e: Env, from: Address, pool_address: Address, amount: i128) -> i128;

    /// Queue deposited pool shares from "from" for withdraw from a backstop of a pool
    ///
    /// Returns the created queue for withdrawal
    ///
    /// ### Arguments
    /// * `from` - The address whose deposits are being queued for withdrawal
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of shares to queue for withdraw
    fn queue_withdrawal(e: Env, from: Address, pool_address: Address, amount: i128) -> Q4W;

    /// Dequeue a currently queued pool share withdraw for "form" from the backstop of a pool
    ///
    /// ### Arguments
    /// * `from` - The address whose deposits are being queued for withdrawal
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of shares to dequeue
    fn dequeue_withdrawal(e: Env, from: Address, pool_address: Address, amount: i128);

    /// Withdraw shares from "from"s withdraw queue for a backstop of a pool
    ///
    /// Returns the amount of tokens returned
    ///
    /// ### Arguments
    /// * `from` - The address whose shares are being withdrawn
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of shares to withdraw
    fn withdraw(e: Env, from: Address, pool_address: Address, amount: i128) -> i128;

    /// Fetch the balance of backstop shares of a pool for the user
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `user` - The user to fetch the balance for
    fn user_balance(e: Env, pool: Address, user: Address) -> UserBalance;

    /// Fetch the backstop data for the pool
    ///
    /// Return a summary of the pool's backstop data
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    fn pool_data(e: Env, pool: Address) -> PoolBackstopData;

    /// Fetch the backstop token for the backstop
    fn backstop_token(e: Env) -> Address;

    /********** Emissions **********/

    /// Consume emissions from the Emitter and distribute them to backstops and pools in the reward zone
    fn gulp_emissions(e: Env);

    /// Add a pool to the reward zone, and if the reward zone is full, a pool to remove
    ///
    /// ### Arguments
    /// * `to_add` - The address of the pool to add
    /// * `to_remove` - The address of the pool to remove
    ///
    /// ### Errors
    /// If the pool to remove has more tokens, or if distribution occurred in the last 48 hours
    fn add_reward(e: Env, to_add: Address, to_remove: Address);

    /// Consume the emissions for a pool and approve
    fn gulp_pool_emissions(e: Env, pool_address: Address) -> i128;

    /// Claim backstop deposit emissions from a list of pools for `from`
    ///
    /// Returns the amount of BLND emissions claimed
    ///
    /// ### Arguments
    /// * `from` - The address of the user claiming emissions
    /// * `pool_addresses` - The Vec of addresses to claim backstop deposit emissions from
    /// * `to` - The Address to send to emissions to
    ///
    /// ### Errors
    /// If an invalid pool address is included
    fn claim(e: Env, from: Address, pool_addresses: Vec<Address>, to: Address) -> i128;

    /// Drop initial BLND to a list of addresses through the emitter
    fn drop(e: Env);

    /********** Fund Management *********/

    /// (Only Pool) Take backstop token from a pools backstop
    ///
    /// ### Arguments
    /// * `from` - The address of the pool drawing tokens from the backstop
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of backstop tokens to draw
    /// * `to` - The address to send the backstop tokens to
    ///
    /// ### Errors
    /// If the pool does not have enough backstop tokens, or if the pool does
    /// not authorize the call
    fn draw(e: Env, pool_address: Address, amount: i128, to: Address);

    /// (Only Pool) Sends backstop tokens from "from" to a pools backstop
    ///
    /// NOTE: This is not a deposit, and "from" will permanently lose access to the funds
    ///
    /// ### Arguments
    /// * `from` - tge
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of BLND to add
    ///
    /// ### Errors
    /// If the `pool_address` is not valid, or if the pool does not
    /// authorize the call
    fn donate(e: Env, from: Address, pool_address: Address, amount: i128);

    /// Updates the underlying value of 1 backstop token
    ///
    /// ### Returns
    /// A tuple of (blnd_per_tkn, usdc_per_tkn) of underlying value per backstop token
    ///
    /// ### Errors
    /// If the underlying value is unable to be computed
    fn update_tkn_val(e: Env) -> (i128, i128);
}

/// @dev
/// The contract implementation only manages the authorization / authentication required from the caller(s), and
/// utilizes other modules to carry out contract functionality.
#[contractimpl]
impl Backstop for BackstopContract {
    fn initialize(
        e: Env,
        backstop_token: Address,
        emitter: Address,
        usdc_token: Address,
        blnd_token: Address,
        pool_factory: Address,
        drop_list: Vec<(Address, i128)>,
    ) {
        storage::extend_instance(&e);
        if storage::get_is_init(&e) {
            panic_with_error!(e, BackstopError::AlreadyInitializedError);
        }

        storage::set_backstop_token(&e, &backstop_token);
        storage::set_blnd_token(&e, &blnd_token);
        storage::set_usdc_token(&e, &usdc_token);
        storage::set_pool_factory(&e, &pool_factory);
        // NOTE: For a replacement backstop, this value likely needs to be stored in persistent storage to avoid
        //       an expiration occuring before a backstop swap is finalized.
        storage::set_drop_list(&e, &drop_list);
        storage::set_emitter(&e, &emitter);

        // fetch last distribution time from emitter
        // NOTE: For a replacement backstop, this must be fetched after the swap is completed, but this is
        //       a shortcut for the first backstop.
        let last_distribution_time =
            EmitterClient::new(&e, &emitter).get_last_distro(&e.current_contract_address());
        storage::set_last_distribution_time(&e, &last_distribution_time);

        storage::set_is_init(&e);
    }

    /********** Core **********/

    fn deposit(e: Env, from: Address, pool_address: Address, amount: i128) -> i128 {
        storage::extend_instance(&e);
        from.require_auth();

        let to_mint = backstop::execute_deposit(&e, &from, &pool_address, amount);

        e.events().publish(
            (Symbol::new(&e, "deposit"), pool_address, from),
            (amount, to_mint),
        );
        to_mint
    }

    fn queue_withdrawal(e: Env, from: Address, pool_address: Address, amount: i128) -> Q4W {
        storage::extend_instance(&e);
        from.require_auth();

        let to_queue = backstop::execute_queue_withdrawal(&e, &from, &pool_address, amount);

        e.events().publish(
            (Symbol::new(&e, "queue_withdrawal"), pool_address, from),
            (amount, to_queue.exp),
        );
        to_queue
    }

    fn dequeue_withdrawal(e: Env, from: Address, pool_address: Address, amount: i128) {
        storage::extend_instance(&e);
        from.require_auth();

        backstop::execute_dequeue_withdrawal(&e, &from, &pool_address, amount);

        e.events().publish(
            (Symbol::new(&e, "dequeue_withdrawal"), pool_address, from),
            amount,
        );
    }

    fn withdraw(e: Env, from: Address, pool_address: Address, amount: i128) -> i128 {
        storage::extend_instance(&e);
        from.require_auth();

        let to_withdraw = backstop::execute_withdraw(&e, &from, &pool_address, amount);

        e.events().publish(
            (Symbol::new(&e, "withdraw"), pool_address, from),
            (amount, to_withdraw),
        );
        to_withdraw
    }

    fn user_balance(e: Env, pool: Address, user: Address) -> UserBalance {
        storage::get_user_balance(&e, &pool, &user)
    }

    fn pool_data(e: Env, pool: Address) -> PoolBackstopData {
        load_pool_backstop_data(&e, &pool)
    }

    fn backstop_token(e: Env) -> Address {
        storage::get_backstop_token(&e)
    }

    /********** Emissions **********/

    fn gulp_emissions(e: Env) {
        storage::extend_instance(&e);
        let new_tokens_emitted = emissions::gulp_emissions(&e);

        e.events()
            .publish((Symbol::new(&e, "gulp_emissions"),), new_tokens_emitted);
    }

    fn add_reward(e: Env, to_add: Address, to_remove: Address) {
        storage::extend_instance(&e);
        emissions::add_to_reward_zone(&e, to_add.clone(), to_remove.clone());

        e.events()
            .publish((Symbol::new(&e, "rw_zone"),), (to_add, to_remove));
    }

    fn gulp_pool_emissions(e: Env, pool_address: Address) -> i128 {
        storage::extend_instance(&e);
        pool_address.require_auth();
        emissions::gulp_pool_emissions(&e, &pool_address)
    }

    fn claim(e: Env, from: Address, pool_addresses: Vec<Address>, to: Address) -> i128 {
        storage::extend_instance(&e);
        from.require_auth();

        let amount = emissions::execute_claim(&e, &from, &pool_addresses, &to);

        e.events().publish((Symbol::new(&e, "claim"), from), amount);
        amount
    }

    fn drop(e: Env) {
        EmitterClient::new(&e, &storage::get_emitter(&e)).drop(&storage::get_drop_list(&e))
    }

    /********** Fund Management *********/

    fn draw(e: Env, pool_address: Address, amount: i128, to: Address) {
        storage::extend_instance(&e);
        pool_address.require_auth();

        backstop::execute_draw(&e, &pool_address, amount, &to);

        e.events()
            .publish((Symbol::new(&e, "draw"), pool_address), (to, amount));
    }

    fn donate(e: Env, from: Address, pool_address: Address, amount: i128) {
        storage::extend_instance(&e);
        from.require_auth();
        pool_address.require_auth();

        backstop::execute_donate(&e, &from, &pool_address, amount);
        e.events()
            .publish((Symbol::new(&e, "donate"), pool_address, from), amount);
    }

    fn update_tkn_val(e: Env) -> (i128, i128) {
        storage::extend_instance(&e);

        let backstop_token = storage::get_backstop_token(&e);
        let blnd_token = storage::get_blnd_token(&e);
        let usdc_token = storage::get_usdc_token(&e);

        backstop::execute_update_comet_token_value(&e, &backstop_token, &blnd_token, &usdc_token)
    }
}

/// Require that an incoming amount is not negative
///
/// ### Arguments
/// * `amount` - The amount
///
/// ### Errors
/// If the number is negative
pub fn require_nonnegative(e: &Env, amount: i128) {
    if amount.is_negative() {
        panic_with_error!(e, BackstopError::NegativeAmountError);
    }
}
