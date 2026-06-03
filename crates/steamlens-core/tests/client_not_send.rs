use static_assertions::assert_not_impl_all;
use steamlens_core::Client;

assert_not_impl_all!(Client: Send);
assert_not_impl_all!(Client: Sync);

#[test]
fn client_send_sync_bounds_documented() {}
