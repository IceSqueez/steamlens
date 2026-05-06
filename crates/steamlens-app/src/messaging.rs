use std::time::{Duration, Instant};

use iced::widget::{button, column, container, row, text};
use iced::{Color, Element, Length, Padding};

use crate::theme::{C_ACCENT, C_BORDER, C_HOVER, C_SURFACE, C_TEXT_MUTED, C_TEXT_PRIMARY};

const C_WARNING: Color = Color::from_rgb(0.878, 0.722, 0.376);
const C_ERROR: Color = Color::from_rgb(0.863, 0.392, 0.392);
const C_SUCCESS: Color = Color::from_rgb(0.314, 0.980, 0.482);

const C_WARNING_BG: Color = Color::from_rgba(0.40, 0.30, 0.10, 0.55);
const C_WARNING_BORDER: Color = Color::from_rgba(0.85, 0.65, 0.25, 0.55);
const C_ERROR_BG: Color = Color::from_rgba(0.45, 0.12, 0.12, 0.65);
const C_ERROR_BORDER: Color = Color::from_rgba(0.85, 0.30, 0.30, 0.55);
const C_INFO_BG: Color = Color::from_rgba(0.12, 0.25, 0.45, 0.55);
const C_INFO_BORDER: Color = Color::from_rgba(0.40, 0.60, 0.90, 0.45);

const SCANNING_DOT: Color = Color::from_rgb(0.741, 0.576, 0.976);
const CONNECTED_DOT: Color = Color::from_rgb(0.314, 0.980, 0.482);
const OFFLINE_DOT: Color = Color::from_rgb(0.878, 0.722, 0.376);

const TOAST_LIFETIME: Duration = Duration::from_secs(4);
const MAX_VISIBLE_TOASTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BannerSeverity {
    Info,
    Warning,
    Error,
}

impl BannerSeverity {
    fn sort_key(self) -> u8 {
        match self {
            BannerSeverity::Error => 2,
            BannerSeverity::Warning => 1,
            BannerSeverity::Info => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BannerAction {
    pub label: &'static str,
    pub message: crate::Message,
}

#[derive(Debug, Clone)]
pub struct Banner {
    pub id: u32,
    pub severity: BannerSeverity,
    pub body: String,
    pub action: Option<BannerAction>,
    pub dismissible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ToastKind {
    Success,
    Info,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: u32,
    pub kind: ToastKind,
    pub title: String,
    pub body: Option<String>,
    created_at: Instant,
    pub hovered: bool,
}

impl Toast {
    fn is_expired(&self) -> bool {
        if self.hovered {
            return false;
        }
        self.created_at.elapsed() >= TOAST_LIFETIME
    }
}

#[derive(Debug, Clone)]
pub enum FooterStatus {
    Connected {
        games: usize,
        last_sync: Option<Instant>,
    },
    Scanning {
        current: usize,
        total: usize,
        label: String,
    },
    Offline {
        cached_games: usize,
    },
}

pub struct MessagingCenter {
    pub banners: Vec<Banner>,
    pub toasts: Vec<Toast>,
    pub footer: FooterStatus,
    next_id: u32,
}

impl std::fmt::Debug for MessagingCenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessagingCenter")
            .field("banners", &self.banners.len())
            .field("toasts", &self.toasts.len())
            .finish()
    }
}

impl MessagingCenter {
    pub fn new() -> Self {
        Self {
            banners: Vec::new(),
            toasts: Vec::new(),
            footer: FooterStatus::Scanning {
                current: 0,
                total: 0,
                label: "Starting up\u{2026}".to_owned(),
            },
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn push_banner(
        &mut self,
        severity: BannerSeverity,
        body: impl Into<String>,
        action: Option<BannerAction>,
        dismissible: bool,
    ) -> u32 {
        let id = self.alloc_id();
        self.banners.push(Banner {
            id,
            severity,
            body: body.into(),
            action,
            dismissible,
        });
        self.banners
            .sort_unstable_by_key(|b| std::cmp::Reverse(b.severity.sort_key()));
        id
    }

    pub fn dismiss_banner(&mut self, id: u32) {
        self.banners.retain(|b| b.id != id);
    }

    pub fn dismiss_all_banners_by_severity(&mut self, severity: BannerSeverity) {
        self.banners.retain(|b| b.severity != severity);
    }

    pub fn push_toast(&mut self, kind: ToastKind, title: impl Into<String>, body: Option<String>) {
        if self.toasts.len() >= MAX_VISIBLE_TOASTS {
            self.toasts.remove(0);
        }
        let id = self.alloc_id();
        self.toasts.push(Toast {
            id,
            kind,
            title: title.into(),
            body,
            created_at: Instant::now(),
            hovered: false,
        });
    }

    pub fn dismiss_toast(&mut self, id: u32) {
        self.toasts.retain(|t| t.id != id);
    }

    pub fn set_toast_hovered(&mut self, id: u32, hovered: bool) {
        if let Some(t) = self.toasts.iter_mut().find(|t| t.id == id) {
            t.hovered = hovered;
        }
    }

    pub fn tick_toasts(&mut self) {
        self.toasts.retain(|t| !t.is_expired());
    }

    pub fn has_active_toasts(&self) -> bool {
        !self.toasts.is_empty()
    }
}

pub fn banner_stack<'a>(messaging: &'a MessagingCenter) -> Option<Element<'a, crate::Message>> {
    if messaging.banners.is_empty() {
        return None;
    }

    let mut col = column![].spacing(0);
    for banner in &messaging.banners {
        col = col.push(banner_strip(banner));
    }
    Some(col.into())
}

fn banner_strip<'a>(banner: &'a Banner) -> Element<'a, crate::Message> {
    let (text_color, bg_color, border_color) = match banner.severity {
        BannerSeverity::Info => (C_TEXT_PRIMARY, C_INFO_BG, C_INFO_BORDER),
        BannerSeverity::Warning => (C_WARNING, C_WARNING_BG, C_WARNING_BORDER),
        BannerSeverity::Error => (C_ERROR, C_ERROR_BG, C_ERROR_BORDER),
    };

    let glyph = match banner.severity {
        BannerSeverity::Info => "\u{2139}",
        BannerSeverity::Warning => "\u{26A0}",
        BannerSeverity::Error => "\u{26D4}",
    };

    let mut content_row = row![
        text(glyph).size(13).color(text_color),
        text(banner.body.as_str()).size(13).color(text_color),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    if let Some(action) = &banner.action {
        let action_msg = action.message.clone();
        content_row = content_row.push(iced::widget::Space::new().width(Length::Fill));
        content_row = content_row.push(
            button(text(action.label).size(12).color(text_color))
                .on_press(action_msg)
                .padding(Padding::default().left(10).right(10).top(4).bottom(4))
                .style(move |_: &iced::Theme, status| {
                    let hovered = matches!(
                        status,
                        iced::widget::button::Status::Hovered
                            | iced::widget::button::Status::Pressed
                    );
                    iced::widget::button::Style {
                        background: Some(iced::Background::Color(Color {
                            a: if hovered { 0.25 } else { 0.15 },
                            ..text_color
                        })),
                        border: iced::Border {
                            color: Color {
                                a: 0.50,
                                ..text_color
                            },
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        text_color,
                        ..iced::widget::button::Style::default()
                    }
                }),
        );
    }

    if banner.dismissible {
        let banner_id = banner.id;
        if banner.action.is_none() {
            content_row = content_row.push(iced::widget::Space::new().width(Length::Fill));
        }
        content_row = content_row.push(
            button(text("\u{00D7}").size(13).color(Color {
                a: 0.6,
                ..text_color
            }))
            .on_press(crate::Message::DismissBanner(banner_id))
            .padding(Padding::default().left(6).right(6).top(2).bottom(2))
            .style(|_: &iced::Theme, status| {
                let hovered = matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                );
                iced::widget::button::Style {
                    background: Some(iced::Background::Color(if hovered {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                    } else {
                        Color::TRANSPARENT
                    })),
                    border: iced::Border::default(),
                    text_color: C_TEXT_MUTED,
                    ..iced::widget::button::Style::default()
                }
            }),
        );
    }

    container(content_row)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(7).bottom(7))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg_color)),
            border: iced::Border {
                color: border_color,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

pub fn status_footer<'a>(messaging: &'a MessagingCenter) -> Element<'a, crate::Message> {
    let (dot_color, left_content, right_btn) = match &messaging.footer {
        FooterStatus::Connected { games, last_sync } => {
            let sync_text = if let Some(t) = last_sync {
                let secs = t.elapsed().as_secs();
                if secs < 60 {
                    "just now".to_owned()
                } else {
                    format!("{}m ago", secs / 60)
                }
            } else {
                "never".to_owned()
            };
            let label = format!("{games} games \u{00B7} synced {sync_text}");
            let content: Element<'_, crate::Message> =
                text(label).size(12).color(C_TEXT_MUTED).into();
            (CONNECTED_DOT, content, None::<Element<'_, crate::Message>>)
        }

        FooterStatus::Scanning {
            current,
            total,
            label,
        } => {
            let progress_text = if *total > 0 {
                format!("{current} / {total} \u{00B7} {label}")
            } else {
                label.clone()
            };
            let content: Element<'_, crate::Message> =
                text(progress_text).size(12).color(C_TEXT_MUTED).into();
            (SCANNING_DOT, content, None::<Element<'_, crate::Message>>)
        }

        FooterStatus::Offline { cached_games } => {
            let label = format!("{cached_games} cached games \u{00B7} offline mode");
            let content: Element<'_, crate::Message> =
                text(label).size(12).color(C_TEXT_MUTED).into();
            let btn = button(text("Reconnect").size(11).color(C_WARNING))
                .on_press(crate::Message::RetrySteamConnect)
                .padding(Padding::default().left(10).right(10).top(3).bottom(3))
                .style(|_: &iced::Theme, status| {
                    let hovered = matches!(
                        status,
                        iced::widget::button::Status::Hovered
                            | iced::widget::button::Status::Pressed
                    );
                    iced::widget::button::Style {
                        background: Some(iced::Background::Color(Color {
                            a: if hovered { 0.22 } else { 0.12 },
                            ..C_WARNING
                        })),
                        border: iced::Border {
                            color: Color {
                                a: 0.45,
                                ..C_WARNING
                            },
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        text_color: C_WARNING,
                        ..iced::widget::button::Style::default()
                    }
                });
            (OFFLINE_DOT, content, Some(btn.into()))
        }
    };

    let dot = container(iced::widget::Space::new())
        .width(Length::Fixed(7.0))
        .height(Length::Fixed(7.0))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(dot_color)),
            border: iced::Border {
                radius: 3.5.into(),
                ..iced::Border::default()
            },
            ..Default::default()
        });

    let mut footer_row = row![dot, left_content]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

    if let Some(btn) = right_btn {
        footer_row = footer_row.push(iced::widget::Space::new().width(Length::Fill));
        footer_row = footer_row.push(btn);
    }

    container(footer_row)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(8).bottom(8))
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                color: C_BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

pub fn toast_stack<'a>(messaging: &'a MessagingCenter) -> Element<'a, crate::Message> {
    let mut toast_col = column![].spacing(8).width(Length::Fixed(320.0));

    for toast in messaging.toasts.iter().rev().take(MAX_VISIBLE_TOASTS) {
        toast_col = toast_col.push(toast_card(toast));
    }

    let overlay_col = column![
        iced::widget::Space::new().height(Length::Fill),
        container(toast_col)
            .width(Length::Fill)
            .padding(Padding::default().right(16).bottom(16)),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::Alignment::End);

    overlay_col.into()
}

fn toast_card<'a>(toast: &'a Toast) -> Element<'a, crate::Message> {
    let (accent_color, kind_glyph) = match toast.kind {
        ToastKind::Success => (C_SUCCESS, "\u{2713}"),
        ToastKind::Info => (C_ACCENT, "\u{2139}"),
    };

    let elapsed_ratio =
        (toast.created_at.elapsed().as_secs_f32() / TOAST_LIFETIME.as_secs_f32()).clamp(0.0, 1.0);
    let remaining_ratio = 1.0 - elapsed_ratio;

    let progress_fill = container(iced::widget::Space::new())
        .width(Length::FillPortion((remaining_ratio * 1000.0) as u16))
        .height(Length::Fixed(2.0))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color {
                a: 0.6,
                ..accent_color
            })),
            border: iced::Border {
                radius: 1.0.into(),
                ..iced::Border::default()
            },
            ..Default::default()
        });

    let progress_remainder = container(iced::widget::Space::new())
        .width(Length::FillPortion(
            ((elapsed_ratio) * 1000.0).max(0.0) as u16
        ))
        .height(Length::Fixed(2.0))
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            ..Default::default()
        });

    let progress_bar = row![progress_fill, progress_remainder]
        .width(Length::Fill)
        .height(Length::Fixed(2.0));

    let title_row = row![
        text(kind_glyph).size(13).color(accent_color),
        text(toast.title.as_str()).size(13).color(C_TEXT_PRIMARY),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    let toast_id = toast.id;
    let close_btn = button(text("\u{00D7}").size(12).color(C_TEXT_MUTED))
        .on_press(crate::Message::DismissToast(toast_id))
        .padding(Padding::default().left(4).right(4).top(2).bottom(2))
        .style(|_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(iced::Background::Color(if hovered {
                    C_HOVER
                } else {
                    Color::TRANSPARENT
                })),
                border: iced::Border::default(),
                text_color: C_TEXT_MUTED,
                ..iced::widget::button::Style::default()
            }
        });

    let header = row![title_row, close_btn]
        .spacing(4)
        .align_y(iced::Alignment::Center);

    let mut card_col = column![header].spacing(4);

    if let Some(body) = &toast.body {
        card_col = card_col.push(text(body.as_str()).size(12).color(C_TEXT_MUTED));
    }

    card_col = card_col.push(progress_bar);

    let toast_id_enter = toast.id;
    let toast_id_exit = toast.id;

    let inner = container(card_col)
        .width(Length::Fill)
        .padding(Padding::default().left(14).right(10).top(10).bottom(10))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                color: Color {
                    a: 0.55,
                    ..accent_color
                },
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        });

    iced::widget::mouse_area(inner)
        .on_enter(crate::Message::ToastHovered(toast_id_enter, true))
        .on_exit(crate::Message::ToastHovered(toast_id_exit, false))
        .into()
}

pub fn wrap_with_messaging<'a>(
    content: Element<'a, crate::Message>,
    messaging: &'a MessagingCenter,
) -> Element<'a, crate::Message> {
    let footer = status_footer(messaging);

    let col_with_footer = if let Some(banners) = banner_stack(messaging) {
        column![banners, content, footer]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
    } else {
        column![content, footer]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
    };

    if messaging.has_active_toasts() {
        let overlay = toast_stack(messaging);
        iced::widget::stack![col_with_footer, overlay].into()
    } else {
        col_with_footer.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messaging_center_starts_empty_banners_and_toasts() {
        let mc = MessagingCenter::new();
        assert!(mc.banners.is_empty());
        assert!(mc.toasts.is_empty());
    }

    #[test]
    fn push_banner_returns_unique_ids() {
        let mut mc = MessagingCenter::new();
        let id1 = mc.push_banner(BannerSeverity::Info, "a", None, false);
        let id2 = mc.push_banner(BannerSeverity::Warning, "b", None, false);
        assert_ne!(id1, id2);
        assert_eq!(mc.banners.len(), 2);
    }

    #[test]
    fn banners_sorted_highest_severity_first() {
        let mut mc = MessagingCenter::new();
        mc.push_banner(BannerSeverity::Info, "info", None, false);
        mc.push_banner(BannerSeverity::Error, "error", None, false);
        mc.push_banner(BannerSeverity::Warning, "warn", None, false);
        assert_eq!(mc.banners[0].severity, BannerSeverity::Error);
        assert_eq!(mc.banners[1].severity, BannerSeverity::Warning);
        assert_eq!(mc.banners[2].severity, BannerSeverity::Info);
    }

    #[test]
    fn dismiss_banner_removes_by_id() {
        let mut mc = MessagingCenter::new();
        let id = mc.push_banner(BannerSeverity::Warning, "test", None, true);
        assert_eq!(mc.banners.len(), 1);
        mc.dismiss_banner(id);
        assert!(mc.banners.is_empty());
    }

    #[test]
    fn toast_queue_caps_at_max_visible() {
        let mut mc = MessagingCenter::new();
        for i in 0..5 {
            mc.push_toast(ToastKind::Info, format!("toast {i}"), None);
        }
        assert_eq!(mc.toasts.len(), MAX_VISIBLE_TOASTS);
    }

    #[test]
    fn toast_queue_drops_oldest_when_full() {
        let mut mc = MessagingCenter::new();
        for i in 0..MAX_VISIBLE_TOASTS {
            mc.push_toast(ToastKind::Info, format!("toast {i}"), None);
        }
        mc.push_toast(ToastKind::Info, "new".to_owned(), None);
        assert_eq!(mc.toasts.len(), MAX_VISIBLE_TOASTS);
        assert_eq!(mc.toasts.last().unwrap().title, "new");
    }

    #[test]
    fn dismiss_toast_removes_by_id() {
        let mut mc = MessagingCenter::new();
        mc.push_toast(ToastKind::Success, "done", None);
        let id = mc.toasts[0].id;
        mc.dismiss_toast(id);
        assert!(mc.toasts.is_empty());
    }

    #[test]
    fn tick_toasts_removes_expired() {
        let mut mc = MessagingCenter::new();
        mc.toasts.push(Toast {
            id: 99,
            kind: ToastKind::Info,
            title: "old".to_owned(),
            body: None,
            created_at: Instant::now() - Duration::from_secs(10),
            hovered: false,
        });
        mc.tick_toasts();
        assert!(mc.toasts.is_empty());
    }

    #[test]
    fn hovered_toast_not_expired() {
        let mut mc = MessagingCenter::new();
        mc.toasts.push(Toast {
            id: 100,
            kind: ToastKind::Info,
            title: "hover me".to_owned(),
            body: None,
            created_at: Instant::now() - Duration::from_secs(10),
            hovered: true,
        });
        mc.tick_toasts();
        assert_eq!(mc.toasts.len(), 1);
    }

    #[test]
    fn set_toast_hovered_updates_flag() {
        let mut mc = MessagingCenter::new();
        mc.push_toast(ToastKind::Info, "test", None);
        let id = mc.toasts[0].id;
        mc.set_toast_hovered(id, true);
        assert!(mc.toasts[0].hovered);
        mc.set_toast_hovered(id, false);
        assert!(!mc.toasts[0].hovered);
    }

    #[test]
    fn footer_status_variants_construct() {
        let _ = FooterStatus::Connected {
            games: 100,
            last_sync: None,
        };
        let _ = FooterStatus::Scanning {
            current: 5,
            total: 100,
            label: "Loading".to_owned(),
        };
        let _ = FooterStatus::Offline { cached_games: 42 };
    }
}
