//! Compile-time guards: `UserStats` wraps a thread-local Steam interface
//! pointer; moving it across threads would call Steam FFI from the wrong
//! thread (UB per the Steam threading model).

use static_assertions::assert_not_impl_all;
use steamlens_core::UserStats;

assert_not_impl_all!(UserStats<'static>: Send);
assert_not_impl_all!(UserStats<'static>: Sync);

#[test]
fn user_stats_send_sync_bounds_documented() {}
