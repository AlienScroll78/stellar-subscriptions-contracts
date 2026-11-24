//! `interfaces::Plan` mirrors the plan crate's `Plan`. A value encoded as the
//! plan crate's type must decode as the mirror with every field intact, so a
//! field added to one side only fails here.

use soroban_sdk::{
    testutils::Address as _, xdr::ScVal, Address, Env, String, TryFromVal, Val,
};

use crate::interfaces::Plan as Mirror;

#[test]
fn plan_mirror() {
    let env = Env::default();
    let merchant = Address::generate(&env);
    let token = Address::generate(&env);
    let name = String::from_str(&env, "Pro");
    let desc = String::from_str(&env, "Priority support");

    let original = plan::Plan {
        id: 7,
        merchant: merchant.clone(),
        name: name.clone(),
        token: token.clone(),
        amount_per_period: 1_000,
        interval_ledgers: 17_280,
        default_cap: 12_000,
        active: true,
        created_at: 42,
        description: Some(desc.clone()),
    };

    let val = Val::try_from_val(&env, &original).unwrap();
    let xdr = ScVal::try_from_val(&env, &val).unwrap();
    let back = Val::try_from_val(&env, &xdr).unwrap();
    let mirror = Mirror::try_from_val(&env, &back).unwrap();

    assert_eq!(mirror.id, 7);
    assert_eq!(mirror.merchant, merchant);
    assert_eq!(mirror.name, name);
    assert_eq!(mirror.token, token);
    assert_eq!(mirror.amount_per_period, 1_000);
    assert_eq!(mirror.interval_ledgers, 17_280);
    assert_eq!(mirror.default_cap, 12_000);
    assert!(mirror.active);
    assert_eq!(mirror.created_at, 42);
    assert_eq!(mirror.description, Some(desc));
}
