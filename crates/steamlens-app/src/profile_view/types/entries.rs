use iced::widget::image::Handle as ImageHandle;

use crate::progress_scan::ProgressData;

#[derive(Clone)]
pub struct StoredCapsule {
    pub handle: ImageHandle,
    pub width: u32,
    pub height: u32,
}

impl std::fmt::Debug for StoredCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StoredCapsule({}x{})", self.width, self.height)
    }
}

#[derive(Clone)]
pub enum CapsuleAsset {
    Pending,
    Loaded {
        handle: ImageHandle,
        width: u32,
        height: u32,
    },
    Unavailable,
}

impl std::fmt::Debug for CapsuleAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapsuleAsset::Pending => write!(f, "Pending"),
            CapsuleAsset::Loaded { width, height, .. } => {
                write!(f, "Loaded({width}x{height})")
            }
            CapsuleAsset::Unavailable => write!(f, "Unavailable"),
        }
    }
}

#[derive(Clone)]
pub struct GameEntry {
    pub app_id: u32,
    pub change_number: u32,
    pub last_played: Option<u32>,
    pub name: Option<String>,
    pub capsule: CapsuleAsset,
    pub progress: Option<ProgressData>,
    pub genre: Option<String>,
}

impl GameEntry {
    pub fn is_hydrated(&self) -> bool {
        self.progress.is_some() && !matches!(self.capsule, CapsuleAsset::Pending)
    }
}

impl std::fmt::Debug for GameEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameEntry")
            .field("app_id", &self.app_id)
            .field("name", &self.name)
            .field("progress", &self.progress)
            .finish_non_exhaustive()
    }
}

pub struct TopEntry {
    pub app_id: u32,
    pub game_name: String,
    pub completion_pct: f64,
    pub earned: u32,
    pub total: u32,
}
