//! `cancel` keeps the subscriber's `BySubscriber` index alive, and the
//! allowance it recomputes still covers the subscriber's other subscriptions.

use super::setup::{Setup, CAP};
use crate::storage::DataKey;
use common::DAY_IN_LEDGERS as DAY;

fn index_ttl(s: &Setup) -> u32 {
    s.env.as_contract(&s.client.address, || {
        s.env
            .storage()
            .persistent()
            .get_ttl(&DataKey::BySubscriber(s.subscriber.clone()))
    })
}

#[test]
fn cancel_extends_by_subscriber_ttl_and_keeps_other_allowances() {
    let s = Setup::new();
    let first = s.subscribe();
    let _second = s.subscribe();

    s.advance(2 * DAY);
    let before = index_ttl(&s);

    s.client.cancel(&s.subscriber, &first);

    assert!(
        index_ttl(&s) > before,
        "cancel did not extend the BySubscriber TTL"
    );
    // The other subscription's cap is still approved, not zeroed.
    assert_eq!(s.allowance(), CAP);
}
