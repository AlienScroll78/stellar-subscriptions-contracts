use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig, Ledger},
    token::StellarAssetClient,
    Address, Env, Vec,
};
use subscription::{SubscriptionContract, SubscriptionContractClient, MAX_BATCH};

const HISTORY_SIZE: u32 = 100;

struct Fixture {
    client: SubscriptionContractClient<'static>,
    subscriber: Address,
    merchant: Address,
    ids: Vec<u64>,
}

impl Fixture {
    fn new() -> Self {
        let env = Env::new_with_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        env.mock_all_auths();
        env.ledger().set_sequence_number(10_000);
        // Fixture construction runs many transactions in one host environment.
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let subscriber = Address::generate(&env);
        let merchant = Address::generate(&env);
        let token = env.register_stellar_asset_contract_v2(admin.clone());
        StellarAssetClient::new(&env, &token.address()).mint(&subscriber, &1_000_000);

        let contract = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract);
        client.initialize(&admin);

        let mut ids = Vec::new(&env);
        for _ in 0..HISTORY_SIZE {
            ids.push_back(client.subscribe(
                &subscriber,
                &merchant,
                &token.address(),
                &100,
                &1_000,
                &1_200,
                &0,
                &0,
            ));
        }

        Self {
            client,
            subscriber,
            merchant,
            ids,
        }
    }
}

fn subscription_benchmarks(c: &mut Criterion) {
    c.bench_function("charge", |b| {
        b.iter_batched_ref(
            || {
                let fixture = Fixture::new();
                let id = fixture.ids.last().unwrap();
                (fixture, id)
            },
            |(fixture, id)| fixture.client.charge(&fixture.merchant, id),
            BatchSize::PerIteration,
        );
    });

    c.bench_function("batch_charge/20", |b| {
        b.iter_batched_ref(
            || {
                let fixture = Fixture::new();
                let ids = fixture.ids.slice(HISTORY_SIZE - MAX_BATCH..);
                (fixture, ids)
            },
            |(fixture, ids)| fixture.client.batch_charge(&fixture.merchant, ids),
            BatchSize::PerIteration,
        );
    });

    let fixture = Fixture::new();
    assert_eq!(
        fixture.client.get_by_subscriber(&fixture.subscriber).len(),
        HISTORY_SIZE
    );
    c.bench_function("get_by_subscriber/100", |b| {
        b.iter(|| black_box(fixture.client.get_by_subscriber(&fixture.subscriber)));
    });
}

criterion_group!(benches, subscription_benchmarks);
criterion_main!(benches);
