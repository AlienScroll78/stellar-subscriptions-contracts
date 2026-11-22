//! `allowance::sync` during `cancel` for a subscriber with many subscriptions.

use super::setup::{Setup, CAP};

#[test]
fn cancel_with_fifty_subscriptions_syncs_the_allowance() {
    let s = Setup::new();
    let ids: std::vec::Vec<u64> = (0..50).map(|_| s.subscribe()).collect();
    assert_eq!(s.allowance(), 50 * CAP);

    s.client.cancel(&s.subscriber, &ids[0]);

    // sync ran to completion: the allowance equals the cap still chargeable
    // across the 49 remaining subscriptions.
    assert_eq!(s.allowance(), 49 * CAP);
}
