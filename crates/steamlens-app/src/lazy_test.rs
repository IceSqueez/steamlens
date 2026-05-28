use iced::widget::lazy;
use iced::{Element, text};
pub fn test<'a>(deps: (u32, bool)) -> Element<'a, ()> {
    lazy(deps, |_| text("test").into()).into()
}
