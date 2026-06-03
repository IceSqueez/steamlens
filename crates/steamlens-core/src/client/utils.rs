use core::ffi::c_void;

use crate::error::SteamError;
use crate::ffi::interfaces::ISteamUtils010;
use crate::ffi::opaque::{self, RawInterface};

pub(super) const STEAM_UTILS_VERSION: &str = "SteamUtils010";

#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub(super) struct Utils {
    pub(super) steam_utils: RawInterface,
}

impl Utils {
    pub(super) fn get_image(&self, handle: i32) -> Result<Option<Image>, SteamError> {
        if handle == 0 {
            return Ok(None);
        }
        if self.steam_utils.is_null() {
            return Err(SteamError::InterfaceUnavailable {
                version: STEAM_UTILS_VERSION.to_owned(),
            });
        }

        let mut width: u32 = 0;
        let mut height: u32 = 0;

        // SAFETY: live `ISteamUtils010`, slot 5 = `GetImageSize`; Steam writes
        // through the stack pointers only on returning `true`.
        tracing::trace!(handle, "utils: get_image_size pre");
        let size_ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUtils010>(self.steam_utils);
            ((*vtbl).get_image_size)(self.steam_utils, handle, &mut width, &mut height)
        };
        tracing::trace!(handle, size_ok, width, height, "utils: get_image_size post");
        if !size_ok || width == 0 || height == 0 {
            return Ok(None);
        }

        let pixel_count = width as usize * height as usize;
        let byte_count = pixel_count * 4;
        let mut rgba: Vec<u8> = vec![0u8; byte_count];

        // SAFETY: `rgba` owns `byte_count` initialised bytes; Steam icons top
        // out around 256x256 (262 144 B) so `byte_count as i32` cannot overflow.
        tracing::trace!(handle, byte_count, "utils: get_image_rgba pre");
        let rgba_ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUtils010>(self.steam_utils);
            ((*vtbl).get_image_rgba)(
                self.steam_utils,
                handle,
                rgba.as_mut_ptr(),
                byte_count as i32,
            )
        };
        tracing::trace!(handle, rgba_ok, "utils: get_image_rgba post");
        if !rgba_ok {
            return Ok(None);
        }

        Ok(Some(Image {
            width,
            height,
            rgba,
        }))
    }

    pub(super) fn poll_call_result(
        &self,
        handle: u64,
        expected_callback_id: i32,
        payload_size: usize,
    ) -> Result<Option<Result<Vec<u8>, SteamError>>, SteamError> {
        if self.steam_utils.is_null() {
            return Err(SteamError::InterfaceUnavailable {
                version: STEAM_UTILS_VERSION.to_owned(),
            });
        }

        let mut failed: bool = false;

        // SAFETY: live `ISteamUtils010`; Steam writes through the stack
        // `failed` pointer on completion-with-IO-error.
        let completed = unsafe {
            let vtbl = opaque::vtable::<ISteamUtils010>(self.steam_utils);
            ((*vtbl).is_api_call_completed)(self.steam_utils, handle, &mut failed)
        };

        if !completed {
            return Ok(None);
        }

        if failed {
            return Ok(Some(Err(SteamError::CallFailed {
                method: "APICall(IO failure)",
            })));
        }

        let mut buf: Vec<u8> = vec![0u8; payload_size];

        // SAFETY: `buf` owns `payload_size` initialised bytes; observed
        // payloads top out at ~144 B so the i32 cast is sound.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUtils010>(self.steam_utils);
            ((*vtbl).get_api_call_result)(
                self.steam_utils,
                handle,
                buf.as_mut_ptr().cast::<c_void>(),
                payload_size as i32,
                expected_callback_id,
                &mut failed,
            )
        };

        if !ok || failed {
            return Ok(Some(Err(SteamError::CallFailed {
                method: "GetAPICallResult",
            })));
        }

        Ok(Some(Ok(buf)))
    }
}
