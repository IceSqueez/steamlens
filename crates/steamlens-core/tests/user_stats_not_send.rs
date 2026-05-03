//! Compile-time verification that `steamlens_core::UserStats` is `!Send` and
//! `!Sync`.
//!
//! `UserStats<'a>` wraps a raw Steam interface pointer that is valid only on
//! the thread that created the `Client`. Sending a `UserStats` to another
//! thread would allow calling Steam FFI from the wrong thread, which is
//! undefined behaviour per the Steam threading model.
//!
//! The current implementation enforces this with `PhantomData<*const ()>`.
//! Removing that field will fail this test file's build, surfacing the
//! regression before it ships.

use static_assertions::assert_not_impl_all;
use steamlens_core::UserStats;

assert_not_impl_all!(UserStats<'static>: Send);
assert_not_impl_all!(UserStats<'static>: Sync);

#[test]
fn user_stats_send_sync_bounds_documented() {
    // The real assertions live in the two macro invocations above; this
    // function exists so `cargo test` lists the file in its output.
}
