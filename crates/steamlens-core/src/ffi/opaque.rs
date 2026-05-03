use core::ffi::c_void;

#[repr(C)]
pub struct OpaqueInstance<V: 'static> {
    pub vtable: *const V,
}

pub type RawInterface = *mut c_void;

/// # Safety
///
/// `instance` must point to a Steam interface object whose first machine word
/// is a pointer to a vtable laid out exactly like `V`. Steam dispatches by
/// vtable index — `V` must list its function pointers in the same order as
/// the canonical Steam interface definition for the version named in the
/// `CreateInterface` / `GetISteamX` call that produced `instance`. The
/// returned pointer aliases the vtable embedded in `steamclient.so`, which
/// stays mapped for the process lifetime; the caller must still keep
/// `instance` alive (and not invalidate it via a release-call) until they
/// have stopped dereferencing the returned pointer.
pub unsafe fn vtable<V: 'static>(instance: RawInterface) -> *const V {
    // SAFETY: the Steam ABI for every `ISteamX` interface places a pointer
    // to the C++ vtable at offset 0 of the object. Reinterpreting `instance`
    // as `*const OpaqueInstance<V>` reads that pointer; the caller's
    // contract on this function guarantees both the offset-0 layout and
    // the matching vtable shape. We return the raw vtable pointer rather
    // than a borrow so the caller's `unsafe` scope owns the dereference
    // and the lifetime is explicit at the callsite.
    let opaque = unsafe { &*(instance as *const OpaqueInstance<V>) };
    opaque.vtable
}
