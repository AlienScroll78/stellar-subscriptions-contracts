//! End to end: `merchant_end` reports `MerchantEnded` (4) to a real registry,
//! and the registry mirrors it instead of rejecting it as `InvalidStatus`.

use registry::{RegistryContract, RegistryContractClient, STATUS_ACTIVE, STATUS_MERCHANT_ENDED};
use soroban_sdk::{testutils::Address as _, Address};

use super::setup::Setup;

fn linked() -> (Setup, RegistryContractClient<'static>) {
    let s = Setup::new();
    let index = RegistryContractClient::new(&s.env, &s.env.register(RegistryContract, ()));
    index.initialize(&s.admin);
    // The plan writer is not exercised here, so any address will do.
    index.set_writers(&s.admin, &s.client.address, &Address::generate(&s.env));
    s.client.set_registry(&s.admin, &index.address);
    (s, index)
}

#[test]
fn merchant_end_is_mirrored_in_the_registry_as_status_4() {
    let (s, index) = linked();
    let id = s.subscribe();
    assert_eq!(index.get_subscription_record(&id).status, STATUS_ACTIVE);

    // Must not fail with RegistryUpdateFailed.
    s.client.merchant_end(&s.merchant, &id);

    assert_eq!(STATUS_MERCHANT_ENDED, 4);
    assert_eq!(
        index.get_subscription_record(&id).status,
        STATUS_MERCHANT_ENDED
    );
    assert_eq!(index.get_stats().active_subscriptions, 0);
}

#[test]
fn merchant_ended_subscription_is_not_in_get_active_subscriptions() {
    let (s, index) = linked();
    let ended = s.subscribe();
    let kept = s.subscribe();

    s.client.merchant_end(&s.merchant, &ended);

    let active = index.get_active_subscriptions();
    assert!(!active.contains(ended));
    assert!(active.contains(kept));
}
