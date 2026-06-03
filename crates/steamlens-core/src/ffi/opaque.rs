use core::ffi::c_void;

#[repr(C)]
pub struct OpaqueInstance<V: 'static> {
    pub vtable: *const V,
}

pub type RawInterface = *mut c_void;

pub unsafe fn vtable<V: 'static>(instance: RawInterface) -> *const V {
    // SAFETY: every `ISteamX` places its vtable pointer at offset 0;
    // the caller's contract pins layout and `V` correspondence.
    let opaque = unsafe { &*(instance as *const OpaqueInstance<V>) };
    opaque.vtable
}
