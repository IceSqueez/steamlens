use std::time::{Duration, Instant};

use iced::widget::{column, container};
use iced::{Element, Length, Padding};

const TOAST_LIFETIME: Duration = Duration::from_secs(4);
const MAX_VISIBLE_TOASTS: usize = 5;

#[derive(Debug, Clone)]
pub enum MessagingEvent {
    ToastTick,
    ToastHovered(u32, bool),
    ToastDismissed(u32),
    BannerDismissed(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerSeverity {
    Info,
    Warning,
    #[allow(
        dead_code,
        reason = "kept for future critical-state banners; Error now routes to toast"
    )]
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

pub struct MessagingCenter {
    pub banners: Vec<Banner>,
    pub toasts: Vec<Toast>,
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
    let mut col = column![]
        .spacing(8)
        .padding(Padding::default().left(12).right(12).top(8));
    for banner in &messaging.banners {
        col = col.push(render_banner(banner));
    }
    Some(col.into())
}

fn render_banner(banner: &Banner) -> Element<'_, crate::Message> {
    use crate::ui::widgets::banner::{Severity, banner as banner_widget};

    let severity = match banner.severity {
        BannerSeverity::Info => Severity::Info,
        BannerSeverity::Warning => Severity::Warning,
        BannerSeverity::Error => Severity::Error,
    };

    let (title, text_line) = match banner.body.split_once('\n') {
        Some((title, rest)) => (title, Some(rest.trim_start())),
        None => (banner.body.as_str(), None),
    };

    let mut b = banner_widget::<crate::Message>()
        .severity(severity)
        .title(title);
    if let Some(t) = text_line {
        b = b.text(t);
    }
    if let Some(action) = &banner.action {
        b = b.action(action.label, action.message.clone());
    }
    if banner.dismissible {
        b = b.on_dismiss(crate::Message::Messaging(MessagingEvent::BannerDismissed(
            banner.id,
        )));
    }
    b.into()
}

pub fn toast_stack<'a>(messaging: &'a MessagingCenter) -> Element<'a, crate::Message> {
    let mut toast_col = column![].spacing(8).width(Length::Fixed(360.0));
    for toast in messaging.toasts.iter().rev().take(MAX_VISIBLE_TOASTS) {
        toast_col = toast_col.push(render_toast(toast));
    }
    container(toast_col)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Alignment::End)
        .align_y(iced::Alignment::Start)
        .padding(Padding::default().right(16).top(16))
        .into()
}

fn render_toast(toast: &Toast) -> Element<'_, crate::Message> {
    use crate::ui::widgets::toast::{Kind, toast as toast_widget};
    let kind = match toast.kind {
        ToastKind::Success => Kind::Success,
        ToastKind::Info => Kind::Info,
        ToastKind::Error => Kind::Error,
    };
    let mut t = toast_widget::<crate::Message>()
        .kind(kind)
        .title(toast.title.clone())
        .on_close(crate::Message::Messaging(MessagingEvent::ToastDismissed(
            toast.id,
        )))
        .on_hover_enter(crate::Message::Messaging(MessagingEvent::ToastHovered(
            toast.id, true,
        )))
        .on_hover_exit(crate::Message::Messaging(MessagingEvent::ToastHovered(
            toast.id, false,
        )));
    if let Some(body) = &toast.body {
        t = t.body(body.clone());
    }
    if let Some(action) = &toast.action {
        t = t.action(action.label.clone(), action.on_press.clone());
    }
    t.into()
}

pub fn wrap_with_toasts<'a>(
    content: Element<'a, crate::Message>,
    messaging: &'a MessagingCenter,
) -> Element<'a, crate::Message> {
    if messaging.has_active_toasts() {
        let overlay = toast_stack(messaging);
        iced::widget::stack![content, overlay].into()
    } else {
        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn push_toast_with_action_carries_action() {
        let mut mc = MessagingCenter::new();
        mc.push_toast_with_action(
            ToastKind::Error,
            "Failed to load app 570",
            Some("Scan failed for app 570".to_owned()),
            ToastAction {
                label: "Retry".to_owned(),
                on_press: crate::Message::Messaging(MessagingEvent::ToastTick),
            },
        );
        assert_eq!(mc.toasts.len(), 1);
        let action = mc.toasts[0].action.as_ref().unwrap();
        assert_eq!(action.label, "Retry");
    }
}
