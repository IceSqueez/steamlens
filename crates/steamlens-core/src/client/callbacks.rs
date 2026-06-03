use core::ptr::addr_of;

use crate::error::SteamError;
use crate::ffi::interfaces::{CallbackMessage, HSteamPipe};
use crate::ffi::loader;
use crate::steam_callback::{SteamCallback, callback_decode};

pub(super) struct Callbacks {
    pub(super) pipe: HSteamPipe,
}

impl Callbacks {
    pub(super) fn poll_callbacks(&self) -> Result<Vec<SteamCallback>, SteamError> {
        let library = loader::shared()?;
        let mut callbacks = Vec::new();

        loop {
            let mut msg = CallbackMessage {
                user: 0,
                id: 0,
                param_ptr: core::ptr::null_mut(),
                param_size: 0,
            };

            // SAFETY: live `pipe` on the `!Send` owner thread; Steam writes
            // through `msg` only on returning `true`; null `call_handle` skips
            // API-call tracking.
            let has_callback =
                library.b_get_callback(self.pipe, &mut msg, core::ptr::null_mut())?;
            if !has_callback {
                break;
            }

            // SAFETY: `msg` is `#[repr(packed)]`; taking a reference to a
            // packed field is UB, so we read each field unaligned.
            let id = unsafe { addr_of!(msg.id).read_unaligned() };
            let param_ptr = unsafe { addr_of!(msg.param_ptr).read_unaligned() };
            let param_size = unsafe { addr_of!(msg.param_size).read_unaligned() };

            let payload = if !param_ptr.is_null() && param_size > 0 {
                // SAFETY: Steam owns `param_ptr` for at least `param_size`
                // bytes until `free_last_callback`; we copy immediately.
                unsafe { core::slice::from_raw_parts(param_ptr, param_size as usize).to_vec() }
            } else {
                Vec::new()
            };

            library.free_last_callback(self.pipe)?;

            callbacks.push(callback_decode::decode(crate::raw_callback::RawCallback {
                id,
                payload,
            }));
        }

        Ok(callbacks)
    }
}
