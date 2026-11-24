#![no_main]
//! Calls subscribe, charge, cancel, pause and resume with arbitrary
//! arguments. Errors are expected and ignored; a panic that escapes the
//! contract (other than a contract error) or a broken invariant is a crash.

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};
use subscription::{SubscriptionContract, SubscriptionContractClient};

const PARTY_COUNT: usize = 4;

#[derive(Debug, Arbitrary)]
enum Op {
    Subscribe {
        amount: i128,
        interval: u32,
        cap: i128,
        plan_id: u64,
        subscriber_idx: u8,
        merchant_idx: u8,
    },
    Charge { id: u64, merchant_idx: u8 },
    Cancel { id: u64, subscriber_idx: u8 },
    Pause { id: u64, subscriber_idx: u8 },
    Resume { id: u64, subscriber_idx: u8 },
    Advance { ledgers: u32 },
    BatchCharge { ids: Vec<u64>, merchant_idx: u8 },
    SubscribeUntil {
        amount: i128,
        interval: u32,
        cap: i128,
        expiry: u32,
        subscriber_idx: u8,
        merchant_idx: u8,
    },
    SubscribeWithTrial {
        amount: i128,
        interval: u32,
        cap: i128,
        trial_ledgers: u32,
        subscriber_idx: u8,
        merchant_idx: u8,
    },
}

fuzz_target!(|ops: Vec<Op>| {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let subscribers: Vec<Address> = (0..PARTY_COUNT)
        .map(|_| Address::generate(&env))
        .collect();
    let merchants: Vec<Address> = (0..PARTY_COUNT)
        .map(|_| Address::generate(&env))
        .collect();
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin).address();
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    for subscriber in &subscribers {
        token_client.mint(subscriber, &1_000_000_000);
    }

    let id = env.register(SubscriptionContract, ());
    let client = SubscriptionContractClient::new(&env, &id);
    client.initialize(&admin);
    let mut subscriptions: Vec<(u64, usize)> = Vec::new();

    for op in ops.into_iter().take(64) {
        match op {
            Op::Subscribe {
                amount,
                interval,
                cap,
                plan_id,
                subscriber_idx,
                merchant_idx,
            } => {
                let subscriber_idx = subscriber_idx as usize % PARTY_COUNT;
                let merchant_idx = merchant_idx as usize % PARTY_COUNT;
                let prior_caps = snapshot_caps(&client, &subscriptions);
                if let Ok(Ok(subscription_id)) = client.try_subscribe(
                    &subscribers[subscriber_idx],
                    &merchants[merchant_idx],
                    &token,
                    &amount,
                    &interval,
                    &cap,
                    &plan_id,
                ) {
                    subscriptions.push((subscription_id, subscriber_idx));
                }
                assert_caps_unchanged(&client, &subscriptions, None, &prior_caps);
            }
            Op::Charge { id, merchant_idx } => {
                let merchant_idx = merchant_idx as usize % PARTY_COUNT;
                let _ = client.try_charge(&merchants[merchant_idx], &id);
            }
            Op::Cancel { id, subscriber_idx } => {
                let subscriber_idx = subscriber_idx as usize % PARTY_COUNT;
                let prior_caps = snapshot_caps(&client, &subscriptions);
                let _ = client.try_cancel(&subscribers[subscriber_idx], &id);
                assert_caps_unchanged(
                    &client,
                    &subscriptions,
                    Some(subscriber_idx),
                    &prior_caps,
                );
            }
            Op::Pause { id, subscriber_idx } => {
                let subscriber_idx = subscriber_idx as usize % PARTY_COUNT;
                let prior_caps = snapshot_caps(&client, &subscriptions);
                let _ = client.try_pause(&subscribers[subscriber_idx], &id);
                assert_caps_unchanged(
                    &client,
                    &subscriptions,
                    Some(subscriber_idx),
                    &prior_caps,
                );
            }
            Op::Resume { id, subscriber_idx } => {
                let subscriber_idx = subscriber_idx as usize % PARTY_COUNT;
                let prior_caps = snapshot_caps(&client, &subscriptions);
                let _ = client.try_resume(&subscribers[subscriber_idx], &id);
                assert_caps_unchanged(
                    &client,
                    &subscriptions,
                    Some(subscriber_idx),
                    &prior_caps,
                );
            }
            Op::Advance { ledgers } => {
                env.ledger().with_mut(|l| {
                    l.sequence_number = l.sequence_number.saturating_add(ledgers % 100_000);
                });
            }
            Op::BatchCharge { ids, merchant_idx } => {
                let merchant_idx = merchant_idx as usize % PARTY_COUNT;
                let _ = client.try_batch_charge(&merchants[merchant_idx], &ids);
            }
            Op::SubscribeUntil {
                amount,
                interval,
                cap,
                expiry,
                subscriber_idx,
                merchant_idx,
            } => {
                let subscriber_idx = subscriber_idx as usize % PARTY_COUNT;
                let merchant_idx = merchant_idx as usize % PARTY_COUNT;
                let prior_caps = snapshot_caps(&client, &subscriptions);
                if let Ok(Ok(subscription_id)) = client.try_subscribe_until(
                    &subscribers[subscriber_idx],
                    &merchants[merchant_idx],
                    &token,
                    &amount,
                    &interval,
                    &cap,
                    &expiry,
                ) {
                    subscriptions.push((subscription_id, subscriber_idx));
                }
                assert_caps_unchanged(&client, &subscriptions, None, &prior_caps);
            }
            Op::SubscribeWithTrial {
                amount,
                interval,
                cap,
                trial_ledgers,
                subscriber_idx,
                merchant_idx,
            } => {
                let subscriber_idx = subscriber_idx as usize % PARTY_COUNT;
                let merchant_idx = merchant_idx as usize % PARTY_COUNT;
                let prior_caps = snapshot_caps(&client, &subscriptions);
                if let Ok(Ok(subscription_id)) = client.try_subscribe_with_trial(
                    &subscribers[subscriber_idx],
                    &merchants[merchant_idx],
                    &token,
                    &amount,
                    &interval,
                    &cap,
                    &trial_ledgers,
                ) {
                    subscriptions.push((subscription_id, subscriber_idx));
                }
                assert_caps_unchanged(&client, &subscriptions, None, &prior_caps);
            }
        }
    }

    // Invariant: CAP. No subscription's remaining cap may be negative.
    for (sid, _) in &subscriptions {
        if let Ok(Ok(rem)) = client.try_remaining_cap(sid) {
            assert!(rem >= 0, "remaining cap went negative");
        }
    }
});

fn snapshot_caps(
    client: &SubscriptionContractClient<'_>,
    subscriptions: &[(u64, usize)],
) -> Vec<(u64, i128)> {
    subscriptions
        .iter()
        .map(|(id, _)| (*id, client.remaining_cap(id)))
        .collect()
}

fn assert_caps_unchanged(
    client: &SubscriptionContractClient<'_>,
    subscriptions: &[(u64, usize)],
    acting_subscriber: Option<usize>,
    before: &[(u64, i128)],
) {
    for (id, owner_idx) in subscriptions {
        if acting_subscriber.map_or(false, |actor_idx| actor_idx == *owner_idx) {
            continue;
        }

        if let Some((_, prior_cap)) = before.iter().find(|(prior_id, _)| prior_id == id) {
            assert_eq!(
                client.remaining_cap(id),
                *prior_cap,
                "another subscriber's action changed remaining cap for subscription {id}"
            );
        }
    }
}
