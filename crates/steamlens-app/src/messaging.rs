use std::time::{Duration, Instant};

use iced::widget::{button, column, container, row, text};
use iced::{Color, Element, Length, Padding};

use crate::theme::{C_ACCENT, C_BORDER, C_SURFACE, C_TEXT_MUTED, C_TEXT_PRIMARY};

const C_WARNING: Color = Color::from_rgb(0.878, 0.722, 0.376);
const C_ERROR: Color = Color::from_rgb(0.863, 0.392, 0.392);
const C_SUCCESS: Color = Color::from_rgb(0.314, 0.980, 0.482);

const CONNECTED_DOT: Color = Color::from_rgb(0.427, 0.788, 0.498);
const OFFLINE_DOT: Color = Color::from_rgb(0.941, 0.784, 0.478);
const TOAST_INFO_BLUE: Color = Color::from_rgb(0.373, 0.643, 0.827);
const TOAST_SURFACE: Color = Color::from_rgb(0.165, 0.149, 0.220);

const TOAST_LIFETIME: Duration = Duration::from_secs(4);
const MAX_VISIBLE_TOASTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub enum ToastKind {
    #[allow(dead_code, reason = "available for future success-feedback toasts")]
    Success,
    Info,
    Error,
}

#[derive(Debug, Clone)]
pub struct ToastAction {
    pub label: String,
    pub on_press: crate::Message,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: u32,
    pub kind: ToastKind,
    pub title: String,
    pub body: Option<String>,
    pub action: Option<ToastAction>,
    created_at: Instant,
    pub hovered: bool,
}

impl Toast {
    fn is_expired(&self) -> bool {
        if self.hovered {
            return false;
        }
        if matches!(self.kind, ToastKind::Error) {
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
            action: None,
            created_at: Instant::now(),
            hovered: false,
        });
    }

    pub fn push_toast_with_action(
        &mut self,
        kind: ToastKind,
        title: impl Into<String>,
        body: Option<String>,
        action: ToastAction,
    ) -> u32 {
        if self.toasts.len() >= MAX_VISIBLE_TOASTS {
            self.toasts.remove(0);
        }
        let id = self.alloc_id();
        self.toasts.push(Toast {
            id,
            kind,
            title: title.into(),
            body,
            action: Some(action),
            created_at: Instant::now(),
            hovered: false,
        });
        id
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
    let accent_color = match banner.severity {
        BannerSeverity::Info => C_ACCENT,
        BannerSeverity::Warning => C_WARNING,
        BannerSeverity::Error => C_ERROR,
    };

    let glyph = match banner.severity {
        BannerSeverity::Info => "\u{2139}",
        BannerSeverity::Warning => "\u{26A0}",
        BannerSeverity::Error => "\u{26D4}",
    };

    let bg_color = Color {
        a: 0.08,
        ..accent_color
    };
    let border_color = Color {
        a: 0.30,
        ..accent_color
    };

    let icon = text(glyph).size(14).color(accent_color);

    let (title_text, subtitle_text) = match banner.body.split_once('\n') {
        Some((title, rest)) => (title, Some(rest.trim_start())),
        None => (banner.body.as_str(), None),
    };

    let text_col: Element<'_, crate::Message> = if let Some(sub) = subtitle_text {
        column![
            text(title_text).size(13).color(C_TEXT_PRIMARY),
            text(sub).size(11).color(C_TEXT_MUTED),
        ]
        .spacing(2)
        .into()
    } else {
        text(title_text).size(13).color(C_TEXT_PRIMARY).into()
    };

    let mut content_row = row![icon, text_col]
        .spacing(12)
        .align_y(iced::Alignment::Center);
    content_row = content_row.push(iced::widget::Space::new().width(Length::Fill));

    let filled = matches!(
        banner.severity,
        BannerSeverity::Warning | BannerSeverity::Error
    );
    if let Some(action) = &banner.action {
        let action_msg = action.message.clone();
        content_row = content_row.push(banner_action_button(
            action.label,
            accent_color,
            filled,
            action_msg,
        ));
    }

    if banner.dismissible {
        let banner_id = banner.id;
        content_row = content_row.push(banner_dismiss_button(banner_id));
    }

    let card = container(content_row)
        .width(Length::Fill)
        .padding(Padding::default().left(14).right(14).top(10).bottom(10))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg_color)),
            border: iced::Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    let stripe = container(iced::widget::Space::new())
        .width(Length::Fixed(3.0))
        .height(Length::Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(accent_color)),
            border: iced::Border {
                radius: 1.5.into(),
                ..iced::Border::default()
            },
            ..Default::default()
        });

    container(
        row![stripe, card]
            .spacing(0)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(Padding::default().left(12).right(12).top(8).bottom(0))
    .into()
}

fn banner_action_button<'a>(
    label: &'static str,
    accent_color: Color,
    filled: bool,
    msg: crate::Message,
) -> Element<'a, crate::Message> {
    let (bg_idle, bg_hover) = if filled { (0.15, 0.25) } else { (0.0, 0.10) };
    button(text(label).size(11).color(accent_color))
        .on_press(msg)
        .padding(Padding::default().left(12).right(12).top(4).bottom(4))
        .style(move |_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(iced::Background::Color(Color {
                    a: if hovered { bg_hover } else { bg_idle },
                    ..accent_color
                })),
                border: iced::Border {
                    color: Color {
                        a: 0.40,
                        ..accent_color
                    },
                    width: 1.0,
                    radius: 5.0.into(),
                },
                text_color: accent_color,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

fn banner_dismiss_button<'a>(banner_id: u32) -> Element<'a, crate::Message> {
    button(text("\u{2715}").size(11).color(C_TEXT_MUTED))
        .on_press(crate::Message::DismissBanner(banner_id))
        .padding(Padding::default().left(4).right(4).top(2).bottom(2))
        .style(|_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(iced::Background::Color(if hovered {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                } else {
                    Color::TRANSPARENT
                })),
                border: iced::Border::default(),
                text_color: C_TEXT_MUTED,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

pub fn status_footer<'a>(
    messaging: &'a MessagingCenter,
    failed_count: usize,
) -> Option<Element<'a, crate::Message>> {
    if failed_count == 0 && footer_is_hidden(&messaging.footer) {
        return None;
    }

    let mut left_row = row![].spacing(14).align_y(iced::Alignment::Center);
    let mut right_action: Option<Element<'_, crate::Message>> = None;

    match &messaging.footer {
        FooterStatus::Connected { games, last_sync } => {
            left_row = left_row.push(footer_status_cluster(
                CONNECTED_DOT,
                "Connected",
                C_TEXT_MUTED,
            ));
            left_row = left_row.push(text(format!("{games} games")).size(11).color(C_TEXT_MUTED));
            left_row = left_row.push(text("\u{00B7}").size(11).color(C_TEXT_MUTED));
            let sync_label = match last_sync {
                Some(t) => {
                    let secs = t.elapsed().as_secs();
                    if secs < 60 {
                        "Last sync just now".to_owned()
                    } else {
                        format!("Last sync {}m ago", secs / 60)
                    }
                }
                None => "Last sync never".to_owned(),
            };
            left_row = left_row.push(text(sync_label).size(11).color(C_TEXT_MUTED));
        }

        FooterStatus::Scanning {
            current,
            total,
            label,
        } => {
            left_row = left_row.push(footer_scanning_cluster(label.as_str()));
            if *total > 0 {
                let ratio = (*current as f32 / *total as f32).clamp(0.0, 1.0);
                left_row = left_row.push(scanning_progress_bar(ratio));
                left_row = left_row.push(
                    text(format!("{current} / {total}"))
                        .size(11)
                        .color(C_TEXT_MUTED),
                );
            }
        }

        FooterStatus::Offline { cached_games } => {
            left_row = left_row.push(footer_status_cluster(OFFLINE_DOT, "Offline", OFFLINE_DOT));
            left_row = left_row.push(
                text(format!("Cached: {cached_games} games"))
                    .size(11)
                    .color(C_TEXT_MUTED),
            );
            right_action = Some(footer_link_button(
                "Reconnect",
                C_ACCENT,
                crate::Message::RetrySteamConnect,
            ));
        }
    }

    let mut footer_row = row![left_row.width(Length::Fill)]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

    if failed_count > 0 {
        let retry_label = format!("Retry ({failed_count})");
        footer_row = footer_row.push(footer_link_button(
            retry_label,
            C_WARNING,
            crate::Message::ProfileView(
                crate::profile_view::types::ProfileViewMessage::RetryFailedScans,
            ),
        ));
    } else if let Some(btn) = right_action {
        footer_row = footer_row.push(btn);
    }

    Some(
        container(footer_row)
            .width(Length::Fill)
            .padding(Padding::default().left(14).right(14).top(8).bottom(8))
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(C_SURFACE)),
                border: iced::Border {
                    color: C_BORDER,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into(),
    )
}

fn footer_status_cluster<'a>(
    dot_color: Color,
    label: &'a str,
    label_color: Color,
) -> Element<'a, crate::Message> {
    let dot = container(iced::widget::Space::new())
        .width(Length::Fixed(6.0))
        .height(Length::Fixed(6.0))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(dot_color)),
            border: iced::Border {
                radius: 3.0.into(),
                ..iced::Border::default()
            },
            ..Default::default()
        });
    row![dot, text(label).size(11).color(label_color)]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .into()
}

fn footer_scanning_cluster<'a>(label: &'a str) -> Element<'a, crate::Message> {
    let spinner = container(iced::widget::Space::new())
        .width(Length::Fixed(6.0))
        .height(Length::Fixed(6.0))
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(C_ACCENT)),
            border: iced::Border {
                radius: 3.0.into(),
                ..iced::Border::default()
            },
            ..Default::default()
        });
    row![spinner, text(label).size(11).color(C_ACCENT)]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .into()
}

fn scanning_progress_bar<'a>(ratio: f32) -> Element<'a, crate::Message> {
    let portion_fill = ((ratio * 1000.0).round() as u16).clamp(1, 1000);
    let portion_rest = 1000 - portion_fill;

    let fill = container(iced::widget::Space::new())
        .width(Length::FillPortion(portion_fill))
        .height(Length::Fixed(3.0))
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(C_ACCENT)),
            border: iced::Border {
                radius: 1.5.into(),
                ..iced::Border::default()
            },
            ..Default::default()
        });
    let rest = container(iced::widget::Space::new())
        .width(Length::FillPortion(portion_rest))
        .height(Length::Fixed(3.0))
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            ..Default::default()
        });

    container(
        container(row![fill, rest].width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fixed(3.0))
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.16, 0.14, 0.22, 1.0,
                ))),
                border: iced::Border {
                    radius: 1.5.into(),
                    ..iced::Border::default()
                },
                ..Default::default()
            }),
    )
    .width(Length::Fixed(200.0))
    .into()
}

fn footer_link_button<'a>(
    label: impl Into<String>,
    color: Color,
    msg: crate::Message,
) -> Element<'a, crate::Message> {
    let label = label.into();
    button(text(label).size(11).color(color))
        .on_press(msg)
        .padding(0)
        .style(move |_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                border: iced::Border::default(),
                text_color: if hovered {
                    Color { a: 0.85, ..color }
                } else {
                    color
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

fn footer_is_hidden(footer: &FooterStatus) -> bool {
    match footer {
        FooterStatus::Connected { .. } => true,
        FooterStatus::Scanning { current, total, .. } => *total > 0 && current >= total,
        FooterStatus::Offline { .. } => false,
    }
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
        ToastKind::Info => (TOAST_INFO_BLUE, "\u{1F4CB}"),
        ToastKind::Error => (C_ERROR, "\u{26D4}"),
    };

    let icon = text(kind_glyph).size(14).color(accent_color);

    let title = text(toast.title.as_str()).size(12).color(C_TEXT_PRIMARY);
    let mut info_col = column![title].spacing(1);
    if let Some(body) = &toast.body {
        info_col = info_col.push(text(body.as_str()).size(10).color(C_TEXT_MUTED));
    }

    let mut content_row = row![icon, info_col.width(Length::Fill)]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    if let Some(action) = &toast.action {
        let action_msg = action.on_press.clone();
        content_row = content_row.push(toast_link_button(
            action.label.clone(),
            C_TEXT_MUTED,
            action_msg,
        ));
    } else if matches!(toast.kind, ToastKind::Error) {
        content_row = content_row.push(toast_link_button(
            "Dismiss".to_owned(),
            C_TEXT_MUTED,
            crate::Message::DismissToast(toast.id),
        ));
    }

    let toast_id_enter = toast.id;
    let toast_id_exit = toast.id;

    let card = container(content_row)
        .width(Length::Fill)
        .padding(Padding::default().left(14).right(14).top(10).bottom(10))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(TOAST_SURFACE)),
            border: iced::Border {
                color: Color {
                    a: 0.30,
                    ..accent_color
                },
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.40),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        });

    let stripe = container(iced::widget::Space::new())
        .width(Length::Fixed(3.0))
        .height(Length::Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(accent_color)),
            border: iced::Border {
                radius: 1.5.into(),
                ..iced::Border::default()
            },
            ..Default::default()
        });

    let composed = container(
        row![stripe, card]
            .spacing(0)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill);

    iced::widget::mouse_area(composed)
        .on_enter(crate::Message::ToastHovered(toast_id_enter, true))
        .on_exit(crate::Message::ToastHovered(toast_id_exit, false))
        .into()
}

fn toast_link_button<'a>(
    label: String,
    color: Color,
    msg: crate::Message,
) -> Element<'a, crate::Message> {
    button(text(label).size(10).color(color))
        .on_press(msg)
        .padding(Padding::default().left(4).right(4).top(2).bottom(2))
        .style(move |_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                border: iced::Border::default(),
                text_color: if hovered { C_TEXT_PRIMARY } else { color },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

pub fn wrap_with_messaging<'a>(
    content: Element<'a, crate::Message>,
    messaging: &'a MessagingCenter,
    failed_count: usize,
) -> Element<'a, crate::Message> {
    let footer = status_footer(messaging, failed_count);

    let mut col = column![]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);
    if let Some(banners) = banner_stack(messaging) {
        col = col.push(banners);
    }
    col = col.push(content);
    if let Some(footer) = footer {
        col = col.push(footer);
    }

    if messaging.has_active_toasts() {
        let overlay = toast_stack(messaging);
        iced::widget::stack![col, overlay].into()
    } else {
        col.into()
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
            action: None,
            created_at: Instant::now() - Duration::from_secs(10),
            hovered: false,
        });
        mc.tick_toasts();
        assert!(mc.toasts.is_empty());
    }

    #[test]
    fn error_toast_never_expires_by_lifetime() {
        let mut mc = MessagingCenter::new();
        mc.toasts.push(Toast {
            id: 200,
            kind: ToastKind::Error,
            title: "boom".to_owned(),
            body: None,
            action: None,
            created_at: Instant::now() - Duration::from_secs(60),
            hovered: false,
        });
        mc.tick_toasts();
        assert_eq!(mc.toasts.len(), 1);
    }

    #[test]
    fn hovered_toast_not_expired() {
        let mut mc = MessagingCenter::new();
        mc.toasts.push(Toast {
            id: 100,
            kind: ToastKind::Info,
            title: "hover me".to_owned(),
            body: None,
            action: None,
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

    #[test]
    fn push_toast_with_action_carries_action() {
        let mut mc = MessagingCenter::new();
        mc.push_toast_with_action(
            ToastKind::Error,
            "Failed to load app 570",
            Some("Scan failed for app 570".to_owned()),
            ToastAction {
                label: "Retry".to_owned(),
                on_press: crate::Message::ToastTick,
            },
        );
        assert_eq!(mc.toasts.len(), 1);
        let action = mc.toasts[0].action.as_ref().unwrap();
        assert_eq!(action.label, "Retry");
    }
}
