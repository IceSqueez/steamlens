//! Compile-time verification that `steamlens_core::Client` is `!Send` and
//! `!Sync`.
//!
//! Steam pipe and user handles are owned by the thread that opened them;
//! moving a `Client` to another thread is undefined behaviour. The current
//! implementation enforces this with a `PhantomData<*const ()>` field.
//!
//! `static_assertions::assert_not_impl_all!` evaluates at compile time:
//! removing the `!Send`/`!Sync` marker from `Client` will fail this test
//! file's build, surfacing the regression before it ships.

use static_assertions::assert_not_impl_all;
use steamlens_core::Client;

assert_not_impl_all!(Client: Send);
assert_not_impl_all!(Client: Sync);

#[test]
fn client_send_sync_bounds_documented() {
    // The real assertion lives in the two macro invocations above; this
    // function exists so `cargo test` lists the file in its output.
}
