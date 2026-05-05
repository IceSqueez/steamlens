use core::ffi::c_void;

#[repr(C)]
pub struct OpaqueInstance<V: 'static> {
    pub vtable: *const V,
}

pub type RawInterface = *mut c_void;

/// # Safety
///
/// `instance` must point to a Steam interface object whose first machine
/// word is a vtable pointer; `V`'s field order must match the canonical
/// vtable for the Steam interface version that produced `instance`
/// (Steam dispatches positionally). The caller must keep `instance`
/// alive for as long as the returned pointer is dereferenced.
pub unsafe fn vtable<V: 'static>(instance: RawInterface) -> *const V {
    // SAFETY: every `ISteamX` places its vtable pointer at offset 0;
    // the caller's contract pins layout and `V` correspondence.
    let opaque = unsafe { &*(instance as *const OpaqueInstance<V>) };
    opaque.vtable
}
