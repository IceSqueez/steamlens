pub use steamlens_core::{AchievementData, StatData, StatValue};

#[derive(Debug, Clone)]
pub struct AchievementRow {
    pub data: AchievementData,
    pub is_dirty: bool,
    pub is_revealed: bool,
    pub has_appeared: bool,
    pub card_opacity: f32,
    pub rarity_percent: Option<f32>,
}

impl From<AchievementData> for AchievementRow {
    fn from(data: AchievementData) -> Self {
        Self {
            data,
            is_dirty: false,
            is_revealed: false,
            has_appeared: false,
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
        self.data.is_hidden && !self.data.is_achieved && !self.is_revealed
    }

    pub fn status(&self) -> AchievementStatus {
        if self.data.permission != 0 {
            AchievementStatus::Protected
        } else if self.is_dirty {
            AchievementStatus::Pending
        } else if self.is_spoiler_hidden() {
            AchievementStatus::Hidden
        } else if self.effective_achieved() {
            AchievementStatus::Unlocked
        } else {
            AchievementStatus::Locked
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AchievementStatus {
    Protected,
    Pending,
    Hidden,
    Unlocked,
    Locked,
}

impl AchievementStatus {
    pub fn label(self) -> &'static str {
        match self {
            AchievementStatus::Protected => "Protected",
            AchievementStatus::Pending => "Pending",
            AchievementStatus::Hidden => "Hidden",
            AchievementStatus::Unlocked => "Unlocked",
            AchievementStatus::Locked => "Locked",
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

impl StatRow {
    pub fn set_value(&mut self, new_value: StatValue) {
        self.data.value = new_value;
        self.edit_text = new_value.to_edit_string();
        self.is_dirty = new_value != self.data.original_value;
        self.edit_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_achievement_row(
        is_hidden: bool,
        is_achieved: bool,
        is_revealed: bool,
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
            is_revealed,
            has_appeared: true,
            card_opacity: 1.0,
            rarity_percent: None,
        }
    }

    #[test]
    fn spoiler_hidden_persisted_unlock_overrides_dirty() {
        let row = make_achievement_row(true, true, false, true, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "already-unlocked secret stays visible even when dirty (pending to lock)"
        );
    }

    #[test]
    fn spoiler_hidden_clean_locked_hidden() {
        let row = make_achievement_row(true, false, false, false, 0);
        assert!(
            row.is_spoiler_hidden(),
            "locked+hidden+not-revealed = spoiler"
        );
    }

    #[test]
    fn spoiler_hidden_after_reveal_click() {
        let row = make_achievement_row(true, false, true, false, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "user clicked reveal: spoiler lifted"
        );
    }

    #[test]
    fn spoiler_hidden_non_hidden_achievement() {
        let row = make_achievement_row(false, false, false, false, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "non-hidden achievement is never a spoiler"
        );
    }

    #[test]
    fn spoiler_hidden_dirty_locked_does_not_unspoil() {
        let row = make_achievement_row(true, false, false, true, 0);
        assert!(
            row.is_spoiler_hidden(),
            "pending-unlock on hidden card: still a spoiler until Apply commits"
        );
    }

    #[test]
    fn status_protected_overrides_all() {
        let row = make_achievement_row(false, false, false, true, 1);
        assert_eq!(row.status(), AchievementStatus::Protected);
    }

    #[test]
    fn status_pending_overrides_hidden() {
        let row = make_achievement_row(true, true, false, true, 0);
        assert_eq!(
            row.status(),
            AchievementStatus::Pending,
            "dirty wins over Hidden when achievement was already unlocked"
        );
    }

    #[test]
    fn status_pending_on_hidden_spoiler() {
        let row = make_achievement_row(true, false, false, true, 0);
        assert_eq!(
            row.status(),
            AchievementStatus::Pending,
            "dirty wins over Hidden even on spoiler card so progress is visible"
        );
    }

    #[test]
    fn status_hidden_when_clean_and_secret() {
        let row = make_achievement_row(true, false, false, false, 0);
        assert_eq!(row.status(), AchievementStatus::Hidden);
    }

    #[test]
    fn status_unlocked_persisted() {
        let row = make_achievement_row(false, true, false, false, 0);
        assert_eq!(row.status(), AchievementStatus::Unlocked);
    }

    #[test]
    fn status_locked_default() {
        let row = make_achievement_row(false, false, false, false, 0);
        assert_eq!(row.status(), AchievementStatus::Locked);
    }

    #[test]
    fn status_unlocked_after_revealed_secret() {
        let row = make_achievement_row(true, true, false, false, 0);
        assert_eq!(
            row.status(),
            AchievementStatus::Unlocked,
            "secret naturally revealed by being earned shows Unlocked"
        );
    }
}
