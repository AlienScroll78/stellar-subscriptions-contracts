//! `version()` must report the exported `VERSION` constant.

use soroban_sdk::Env;

use crate::{PlanContract, PlanContractClient, VERSION};

#[test]
fn version_returns_the_version_constant() {
    let env = Env::default();
    let client = PlanContractClient::new(&env, &env.register(PlanContract, ()));
    assert_eq!(client.version(), VERSION);
}
