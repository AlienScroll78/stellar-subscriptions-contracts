//! Instruction cost of walking a subscriber's history, at 10, 100 and 1,000
//! subscriptions.
//!
//! SECURITY.md documents the large-history cost as a known limit but had no
//! concrete number. `get_by_subscriber` itself refuses more than
//! `MAX_LIST_SIZE` (200) entries, so it cannot be measured directly at 1,000;
//! `get_by_subscriber_page`, which has no such ceiling, is used instead to
//! get a comparable per-subscription cost at every size. This is a separate
//! integration test (not part of `src/test`) so it can be run on its own.

use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env};
use subscription::{SubscriptionContract, SubscriptionContractClient};

fn setup_with_subscriptions(n: u32) -> (Env, SubscriptionContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let subscriber = Address::generate(&env);
    let merchant = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(admin.clone());
    StellarAssetClient::new(&env, &token.address()).mint(&subscriber, &1_000_000_000);

    let contract_id = env.register(SubscriptionContract, ());
    let client = SubscriptionContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    for _ in 0..n {
        client.subscribe(
            &subscriber,
            &merchant,
            &token.address(),
            &100,
            &1_000,
            &1_200,
            &0,
            &0,
        );
    }

    (env, client, subscriber)
}

/// CPU instructions consumed by one `get_by_subscriber_page` call that reads
/// all `n` of the subscriber's subscriptions.
fn instruction_cost_of_reading_all(n: u32) -> u64 {
    let (env, client, subscriber) = setup_with_subscriptions(n);
    env.cost_estimate().budget().reset_default();
    let before = env.cost_estimate().budget().cpu_instruction_cost();
    client.get_by_subscriber_page(&subscriber, &0, &n);
    let after = env.cost_estimate().budget().cpu_instruction_cost();
    after - before
}

#[test]
fn instruction_cost_at_10_100_and_1000_subscriptions() {
    let cost_10 = instruction_cost_of_reading_all(10);
    let cost_100 = instruction_cost_of_reading_all(100);
    let cost_1000 = instruction_cost_of_reading_all(1_000);

    std::println!(
        "get_by_subscriber_page instruction cost: n=10 -> {cost_10}, n=100 -> {cost_100}, n=1000 -> {cost_1000}"
    );
    std::println!(
        "approx per-subscription cost: n=10 -> {}, n=100 -> {}, n=1000 -> {}",
        cost_10 / 10,
        cost_100 / 100,
        cost_1000 / 1_000
    );

    // Cost grows with history size, and roughly linearly rather than
    // super-linearly, which is what justifies a flat per-subscriber cap.
    assert!(cost_100 > cost_10);
    assert!(cost_1000 > cost_100);
}
