//! Модель раскладки: панели, группы, объекты и правила валидации.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

/// Шаг сетки рабочей области.
pub const GRID_STEP: i32 = 8;
/// Базовая единица плитки.
pub const TILE_UNIT: i32 = 88;
/// Текущая версия схемы layout.json.
pub const LAYOUT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelType {
    Apps,
    Ai,
    Status,
    Services,
}

impl PanelType {
    pub fn id(self) -> &'static str {
        match self {
            PanelType::Apps => "apps",
            PanelType::Ai => "ai",
            PanelType::Status => "status",
            PanelType::Services => "services",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelMode {
    Hover,
    Dock,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Panel {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: PanelType,
    pub edge: Edge,
    pub mode: PanelMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default)]
    pub density: Density,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObjectKind {
    App,
    Widget,
    Group,
    Separator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub title: String,
    #[serde(flatten)]
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Object {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ObjectKind,
    #[serde(flatten)]
    pub rect: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Screen {
    pub panels: Vec<Panel>,
    pub groups: Vec<Group>,
    pub objects: Vec<Object>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub version: u32,
    pub monitors: BTreeMap<String, Screen>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LayoutError {
    #[error("unsupported layout version: {0}")]
    UnsupportedVersion(u32),
    #[error("duplicate panel type {0:?}")]
    DuplicatePanelType(PanelType),
    #[error("duplicate panel edge {0:?}")]
    DuplicatePanelEdge(Edge),
    #[error("object {0} is not aligned to the {1} px grid")]
    NotAligned(String, i32),
    #[error("object {0} has invalid size {1}x{2}")]
    InvalidSize(String, i32, i32),
    #[error("objects {0} and {1} overlap")]
    Overlap(String, String),
    #[error("object {0} references missing group {1}")]
    MissingGroup(String, String),
}

impl Layout {
    /// Раскладка по умолчанию для основного монитора.
    pub fn default_primary() -> Self {
        let mut monitors = BTreeMap::new();
        monitors.insert("primary".to_string(), Screen::default_screen());
        Self {
            version: LAYOUT_VERSION,
            monitors,
        }
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("layout serialization cannot fail")
    }

    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.version != LAYOUT_VERSION {
            return Err(LayoutError::UnsupportedVersion(self.version));
        }
        for screen in self.monitors.values() {
            screen.validate()?;
        }
        Ok(())
    }
}

impl Screen {
    pub fn default_screen() -> Self {
        Self {
            panels: vec![
                Panel {
                    id: "apps".into(),
                    kind: PanelType::Apps,
                    edge: Edge::Left,
                    mode: PanelMode::Hover,
                    width: Some(360),
                    height: None,
                    density: Density::Compact,
                },
                Panel {
                    id: "ai".into(),
                    kind: PanelType::Ai,
                    edge: Edge::Right,
                    mode: PanelMode::Hover,
                    width: Some(420),
                    height: None,
                    density: Density::Compact,
                },
                Panel {
                    id: "status".into(),
                    kind: PanelType::Status,
                    edge: Edge::Top,
                    mode: PanelMode::Dock,
                    width: None,
                    height: Some(44),
                    density: Density::Compact,
                },
                Panel {
                    id: "services".into(),
                    kind: PanelType::Services,
                    edge: Edge::Bottom,
                    mode: PanelMode::Dock,
                    width: None,
                    height: Some(48),
                    density: Density::Compact,
                },
            ],
            groups: vec![Group {
                id: "office".into(),
                title: "Офис".into(),
                rect: Rect {
                    x: 16,
                    y: 200,
                    w: 352,
                    h: 176,
                },
            }],
            objects: vec![
                Object {
                    id: "mail".into(),
                    kind: ObjectKind::Widget,
                    rect: Rect {
                        x: 16,
                        y: 96,
                        w: 176,
                        h: 88,
                    },
                    app: None,
                    widget: Some("mail".into()),
                    group_id: None,
                },
                Object {
                    id: "weather".into(),
                    kind: ObjectKind::Widget,
                    rect: Rect {
                        x: 200,
                        y: 96,
                        w: 176,
                        h: 88,
                    },
                    app: None,
                    widget: Some("weather".into()),
                    group_id: None,
                },
                Object {
                    id: "1c".into(),
                    kind: ObjectKind::App,
                    rect: Rect {
                        x: 24,
                        y: 208,
                        w: 88,
                        h: 88,
                    },
                    app: Some("1С".into()),
                    widget: None,
                    group_id: Some("office".into()),
                },
                Object {
                    id: "word".into(),
                    kind: ObjectKind::App,
                    rect: Rect {
                        x: 120,
                        y: 208,
                        w: 88,
                        h: 88,
                    },
                    app: Some("Word".into()),
                    widget: None,
                    group_id: Some("office".into()),
                },
                Object {
                    id: "excel".into(),
                    kind: ObjectKind::App,
                    rect: Rect {
                        x: 216,
                        y: 208,
                        w: 88,
                        h: 88,
                    },
                    app: Some("Excel".into()),
                    widget: None,
                    group_id: Some("office".into()),
                },
            ],
        }
    }

    pub fn validate(&self) -> Result<(), LayoutError> {
        let mut types = HashSet::new();
        let mut edges = HashSet::new();
        for panel in &self.panels {
            if !types.insert(panel.kind) {
                return Err(LayoutError::DuplicatePanelType(panel.kind));
            }
            if !edges.insert(panel.edge) {
                return Err(LayoutError::DuplicatePanelEdge(panel.edge));
            }
        }

        let group_ids: HashSet<&str> = self.groups.iter().map(|g| g.id.as_str()).collect();

        for object in &self.objects {
            if object.rect.x % GRID_STEP != 0 || object.rect.y % GRID_STEP != 0 {
                return Err(LayoutError::NotAligned(object.id.clone(), GRID_STEP));
            }
            if object.rect.w <= 0 || object.rect.h <= 0 {
                return Err(LayoutError::InvalidSize(
                    object.id.clone(),
                    object.rect.w,
                    object.rect.h,
                ));
            }
            if matches!(object.kind, ObjectKind::App | ObjectKind::Widget)
                && (object.rect.w % TILE_UNIT != 0 || object.rect.h % TILE_UNIT != 0)
            {
                return Err(LayoutError::InvalidSize(
                    object.id.clone(),
                    object.rect.w,
                    object.rect.h,
                ));
            }
            if let Some(group_id) = &object.group_id {
                if !group_ids.contains(group_id.as_str()) {
                    return Err(LayoutError::MissingGroup(
                        object.id.clone(),
                        group_id.clone(),
                    ));
                }
            }
        }

        for (i, left) in self.objects.iter().enumerate() {
            for right in self.objects.iter().skip(i + 1) {
                if left.rect.intersects(&right.rect) {
                    return Err(LayoutError::Overlap(left.id.clone(), right.id.clone()));
                }
            }
        }

        for group in &self.groups {
            if group.rect.x % GRID_STEP != 0
                || group.rect.y % GRID_STEP != 0
                || group.rect.w % GRID_STEP != 0
                || group.rect.h % GRID_STEP != 0
            {
                return Err(LayoutError::NotAligned(group.id.clone(), GRID_STEP));
            }
            if group.rect.w <= 0 || group.rect.h <= 0 {
                return Err(LayoutError::InvalidSize(
                    group.id.clone(),
                    group.rect.w,
                    group.rect.h,
                ));
            }
        }

        for (i, left) in self.groups.iter().enumerate() {
            for right in self.groups.iter().skip(i + 1) {
                if left.rect.intersects(&right.rect) {
                    return Err(LayoutError::Overlap(left.id.clone(), right.id.clone()));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_is_valid() {
        assert_eq!(Layout::default_primary().validate(), Ok(()));
    }

    #[test]
    fn default_layout_round_trips_through_json() {
        let layout = Layout::default_primary();
        let json = layout.to_json_pretty();
        let parsed: Layout = serde_json::from_str(&json).expect("parse layout");
        assert_eq!(layout, parsed);
    }

    #[test]
    fn misaligned_object_is_rejected() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        screen.objects[0].rect.x = 18;
        assert_eq!(
            screen.validate(),
            Err(LayoutError::NotAligned("mail".into(), GRID_STEP))
        );
    }

    #[test]
    fn wrong_tile_size_is_rejected() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        screen.objects[0].rect.w = 100;
        assert_eq!(
            screen.validate(),
            Err(LayoutError::InvalidSize("mail".into(), 100, 88))
        );
    }

    #[test]
    fn overlapping_objects_are_rejected() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        let mut extra = screen.objects[0].clone();
        extra.id = "copy".into();
        screen.objects.push(extra);
        assert_eq!(
            screen.validate(),
            Err(LayoutError::Overlap("mail".into(), "copy".into()))
        );
    }

    #[test]
    fn missing_group_is_rejected() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        screen.objects[2].group_id = Some("nope".into());
        assert_eq!(
            screen.validate(),
            Err(LayoutError::MissingGroup("1c".into(), "nope".into()))
        );
    }

    #[test]
    fn duplicate_panel_type_is_rejected() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        let mut extra = screen.panels[0].clone();
        extra.id = "apps2".into();
        extra.edge = Edge::Bottom;
        screen.panels.push(extra);
        assert_eq!(
            screen.validate(),
            Err(LayoutError::DuplicatePanelType(PanelType::Apps))
        );
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let mut layout = Layout::default_primary();
        layout.version = 99;
        assert_eq!(layout.validate(), Err(LayoutError::UnsupportedVersion(99)));
    }
}
