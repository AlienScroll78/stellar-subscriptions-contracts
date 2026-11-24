//! Plan and registry deployed together and linked: `create_plan` must be
//! counted in `RegistryStats.total_plans`.

use registry::{RegistryContract, RegistryContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{PlanContract, PlanContractClient};

struct Linked {
    env: Env,
    plans: PlanContractClient<'static>,
    index: RegistryContractClient<'static>,
    merchant: Address,
    token: Address,
}

impl Linked {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let plans = PlanContractClient::new(&env, &env.register(PlanContract, ()));
        let index = RegistryContractClient::new(&env, &env.register(RegistryContract, ()));
        plans.initialize(&admin);
        index.initialize(&admin);
        // The subscription writer is not exercised here, so any address will do.
        index.set_writers(&admin, &Address::generate(&env), &plans.address);
        plans.set_registry(&admin, &index.address);
        Linked {
            merchant: Address::generate(&env),
            token: Address::generate(&env),
            env,
            plans,
            index,
        }
    }

    fn create(&self, name: &str) -> u64 {
        self.plans.create_plan(
            &self.merchant,
            &String::from_str(&self.env, name),
            &self.token,
            &500,
            &17_280,
            &6_000,
            &None,
        )
    }
}

#[test]
fn create_plan_increments_registry_total_plans() {
    let l = Linked::new();
    assert_eq!(l.index.get_stats().total_plans, 0);
    l.create("Pro");
    assert_eq!(l.index.get_stats().total_plans, 1);
}

#[test]
fn second_create_plan_increments_total_plans_to_two() {
    let l = Linked::new();
    l.create("Pro");
    l.create("Team");
    assert_eq!(l.index.get_stats().total_plans, 2);
}

#[test]
fn deactivate_plan_does_not_change_total_plans() {
    let l = Linked::new();
    let id = l.create("Pro");
    l.plans.deactivate_plan(&l.merchant, &id);
    assert_eq!(l.index.get_stats().total_plans, 1);
}
