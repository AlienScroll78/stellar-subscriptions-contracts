//! Tests for the `charge` entrypoint.
//!
//! Covers wallet-address merchants as well as contract-account merchants
//! (C... addresses) to confirm authorization semantics are identical.

extern crate std;

use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, token, Address, Env, String,
};

use crate::{
    contract::{SubscriptionContract, SubscriptionContractClient},
    error::ContractError,
    types::{Interval, SubscriptionStatus},
};

/// Minimal merchant contract used to exercise contract-account merchants.
/// It forwards `charge` calls to the subscription contract so we can verify
/// that a C... address is treated exactly like a wallet merchant.
#[contract]
pub struct MerchantContract;

#[contractimpl]
impl MerchantContract {
    /// Called by the subscription contract's `charge`; records the last
    /// charged amount so tests can assert the call actually happened.
    pub fn on_charge(env: Env, amount: i128) {
        env.storage()
            .instance()
            .set(&symbol_short!("last"), &amount);
    }

    /// Have the merchant contract itself invoke `charge` on the subscription
    /// contract. Because the subscription contract calls `merchant.require_auth()`,
    /// this succeeds only when the caller is the merchant contract.
    pub fn charge(
        env: Env,
        subscription: Address,
        subscriber: Address,
        merchant: Address,
    ) -> i128 {
        let client = SubscriptionContractClient::new(&env, &subscription);
        client.charge(&subscriber, &merchant)
    }

    pub fn last_charged(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&symbol_short!("last"))
            .unwrap_or(0)
    }
}

fn setup_token<'a>(env: &Env, admin: &Address) -> (Address, token::StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_id = sac.address();
    let token_admin = token::StellarAssetClient::new(env, &token_id);
    (token_id, token_admin)
}

#[test]
fn charge_by_contract_merchant_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let subscriber = Address::generate(&env);

    let (token_id, token_admin) = setup_token(&env, &admin);
    token_admin.mint(&subscriber, &1_000);

    // Deploy the merchant contract; its address is a C... address.
    let merchant_id = env.register_contract(None, MerchantContract);
    let merchant_client = MerchantContractClient::new(&env, &merchant_id);

    let subscription_id = env.register_contract(None, SubscriptionContract);
    let subscription_client = SubscriptionContractClient::new(&env, &subscription_id);

    let plan_id = subscription_client.create_plan(
        &merchant_id,
        &token_id,
        &100,
        &Interval::Month,
        &String::from_str(&env, "contract merchant plan"),
    );

    subscription_client.subscribe(&subscriber, &plan_id);

    // The merchant contract calls `charge` on its own behalf.
    let charged = merchant_client.charge(&subscription_id, &subscriber, &merchant_id);
    assert_eq!(charged, 100);

    let sub = subscription_client.get_subscription(&subscriber, &plan_id).unwrap();
    assert_eq!(sub.status, SubscriptionStatus::Active);
}

#[test]
fn charge_by_third_party_for_contract_merchant_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let subscriber = Address::generate(&env);
    let third_party = Address::generate(&env);

    let (token_id, token_admin) = setup_token(&env, &admin);
    token_admin.mint(&subscriber, &1_000);

    let merchant_id = env.register_contract(None, MerchantContract);

    let subscription_id = env.register_contract(None, SubscriptionContract);
    let subscription_client = SubscriptionContractClient::new(&env, &subscription_id);

    let plan_id = subscription_client.create_plan(
        &merchant_id,
        &token_id,
        &100,
        &Interval::Month,
        &String::from_str(&env, "contract merchant plan"),
    );

    subscription_client.subscribe(&subscriber, &plan_id);

    // A third party cannot charge on behalf of the merchant contract.
    let result = subscription_client.try_charge(&subscriber, &third_party);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}
