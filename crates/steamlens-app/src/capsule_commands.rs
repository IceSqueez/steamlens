use iced::Task;
use iced::widget::image::Handle as ImageHandle;

use crate::capsule_cache::{self, CapsuleSize};
use crate::profile_view::types::ProfileViewMessage;

pub fn fetch_capsule(app_id: u32, size: CapsuleSize) -> Task<ProfileViewMessage> {
    Task::perform(
        async move { capsule_cache::fetch_capsule(app_id, size).await },
        move |result| match result {
            Ok((fetched_size, pixels)) => {
                let handle = ImageHandle::from_rgba(pixels.width, pixels.height, pixels.rgba);
                ProfileViewMessage::CapsuleLoaded {
                    app_id,
                    size: fetched_size,
                    handle,
                    width: pixels.width,
                    height: pixels.height,
                }
            }
            Err((fetched_size, _)) => ProfileViewMessage::CapsuleFailed {
                app_id,
                size: fetched_size,
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_capsule_builds() {
        let _: Task<ProfileViewMessage> = fetch_capsule(440, CapsuleSize::Medium);
    }
}
