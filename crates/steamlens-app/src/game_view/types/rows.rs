pub use steamlens_core::{AchievementData, StatData, StatValue};

#[derive(Debug, Clone)]
pub struct AchievementRow {
    pub data: AchievementData,
    pub is_dirty: bool,
    pub revealed: bool,
    pub appeared: bool,
    pub card_opacity: f32,
    pub rarity_percent: Option<f32>,
}

impl From<AchievementData> for AchievementRow {
    fn from(data: AchievementData) -> Self {
        Self {
            data,
            is_dirty: false,
            revealed: false,
            appeared: false,
            card_opacity: 0.0,
            rarity_percent: None,
        }
    }
}

impl AchievementRow {
    pub fn effective_achieved(&self) -> bool {
        if self.is_dirty {
            !self.data.is_achieved
        } else {
            self.data.is_achieved
        }
    }

    pub fn is_spoiler_hidden(&self) -> bool {
        self.data.is_hidden && !self.data.is_achieved && !self.revealed
    }

    pub fn status_label(&self) -> &'static str {
        if self.data.permission != 0 {
            "Protected"
        } else if self.is_dirty {
            "Pending"
        } else if self.is_spoiler_hidden() {
            "Hidden"
        } else if self.effective_achieved() {
            "Unlocked"
        } else {
            "Locked"
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatRow {
    pub data: StatData,
    pub edit_text: String,
    pub edit_error: Option<String>,
    pub is_dirty: bool,
}

impl From<StatData> for StatRow {
    fn from(data: StatData) -> Self {
        let edit_text = data.value.to_edit_string();
        Self {
            data,
            edit_text,
            edit_error: None,
            is_dirty: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(
        is_hidden: bool,
        is_achieved: bool,
        revealed: bool,
        is_dirty: bool,
        permission: u32,
    ) -> AchievementRow {
        AchievementRow {
            data: AchievementData {
                id: "test".to_owned(),
                display_name: "Test".to_owned(),
                description: String::new(),
                is_achieved,
                unlock_time: None,
                is_hidden,
                permission,
                icon: None,
            },
            is_dirty,
            revealed,
            appeared: true,
            card_opacity: 1.0,
            rarity_percent: None,
        }
    }

    #[test]
    fn spoiler_hidden_persisted_unlock_overrides_dirty() {
        let row = make_row(true, true, false, true, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "already-unlocked secret stays visible even when dirty (pending to lock)"
        );
    }

    #[test]
    fn spoiler_hidden_clean_locked_hidden() {
        let row = make_row(true, false, false, false, 0);
        assert!(
            row.is_spoiler_hidden(),
            "locked+hidden+not-revealed = spoiler"
        );
    }

    #[test]
    fn spoiler_hidden_after_reveal_click() {
        let row = make_row(true, false, true, false, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "user clicked reveal: spoiler lifted"
        );
    }

    #[test]
    fn spoiler_hidden_non_hidden_achievement() {
        let row = make_row(false, false, false, false, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "non-hidden achievement is never a spoiler"
        );
    }

    #[test]
    fn spoiler_hidden_dirty_locked_does_not_unspoil() {
        let row = make_row(true, false, false, true, 0);
        assert!(
            row.is_spoiler_hidden(),
            "pending-unlock on hidden card: still a spoiler until Apply commits"
        );
    }

    #[test]
    fn status_label_protected_overrides_all() {
        let row = make_row(false, false, false, true, 1);
        assert_eq!(row.status_label(), "Protected");
    }

    #[test]
    fn status_label_pending_overrides_hidden() {
        let row = make_row(true, true, false, true, 0);
        assert_eq!(
            row.status_label(),
            "Pending",
            "dirty wins over Hidden when achievement was already unlocked"
        );
    }

    #[test]
    fn status_label_pending_on_hidden_spoiler() {
        let row = make_row(true, false, false, true, 0);
        assert_eq!(
            row.status_label(),
            "Pending",
            "dirty wins over Hidden even on spoiler card so progress is visible"
        );
    }

    #[test]
    fn status_label_hidden_when_clean_and_secret() {
        let row = make_row(true, false, false, false, 0);
        assert_eq!(row.status_label(), "Hidden");
    }

    #[test]
    fn status_label_unlocked_persisted() {
        let row = make_row(false, true, false, false, 0);
        assert_eq!(row.status_label(), "Unlocked");
    }

    #[test]
    fn status_label_locked_default() {
        let row = make_row(false, false, false, false, 0);
        assert_eq!(row.status_label(), "Locked");
    }

    #[test]
    fn status_label_unlocked_after_revealed_secret() {
        let row = make_row(true, true, false, false, 0);
        assert_eq!(
            row.status_label(),
            "Unlocked",
            "secret naturally revealed by being earned shows Unlocked"
        );
    }
}
