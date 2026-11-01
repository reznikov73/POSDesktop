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

    /// Попадает ли точка внутрь прямоугольника (правая и нижняя границы исключены).
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Центр прямоугольника.
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

/// Привязка координаты при перетаскивании: шаг сетки [`GRID_STEP`] (ТЗ §6.1).
///
/// Размеры плиток кратны 88 px, а позиции живут на 8-пиксельной сетке — так
/// раскладка остаётся валидной и штатные координаты не «прыгают» при отпускании.
pub fn snap_to_grid(value: i32) -> i32 {
    (value + GRID_STEP / 2).div_euclid(GRID_STEP) * GRID_STEP
}

impl Object {
    /// Подпись плитки: приложение, виджет или пустая строка.
    pub fn label(&self) -> &str {
        self.app
            .as_deref()
            .or(self.widget.as_deref())
            .unwrap_or_default()
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoveError {
    #[error("object {0} not found")]
    NotFound(String),
    #[error("no free position for {0} near ({1}, {2})")]
    NoFreePosition(String, i32, i32),
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
    /// Индекс объекта по идентификатору.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.objects.iter().position(|object| object.id == id)
    }

    /// Куда встанет объект, если его отпустить в точке (x, y).
    ///
    /// Учитывает привязку к сетке и автосдвиг при пересечении, но ничего не
    /// меняет: UI использует метод для предпросмотра (ТЗ §6.5).
    pub fn preview_move(&self, id: &str, x: i32, y: i32) -> Result<Rect, MoveError> {
        let index = self
            .index_of(id)
            .ok_or_else(|| MoveError::NotFound(id.to_string()))?;
        let size = self.objects[index].rect;
        let target = Rect {
            x: snap_to_grid(x.max(0)),
            y: snap_to_grid(y.max(0)),
            w: size.w,
            h: size.h,
        };
        if self.is_free(index, &target) {
            return Ok(target);
        }
        self.nearest_free(index, target)
            .ok_or(MoveError::NoFreePosition(
                id.to_string(),
                target.x,
                target.y,
            ))
    }

    /// Перемещает объект с привязкой к сетке и автосдвигом при пересечении.
    ///
    /// Группа объекта определяется по центру плитки: попала в контейнер —
    /// закрепляется за ним, вышла — отвязывается (ТЗ §6.3).
    pub fn move_object(&mut self, id: &str, x: i32, y: i32) -> Result<Rect, MoveError> {
        let rect = self.preview_move(id, x, y)?;
        if let Some(index) = self.index_of(id) {
            self.objects[index].rect = rect;
            self.objects[index].group_id = self.group_at(rect);
        }
        Ok(rect)
    }

    /// Свободна ли позиция: остальные объекты не пересекаются.
    fn is_free(&self, index: usize, rect: &Rect) -> bool {
        self.objects
            .iter()
            .enumerate()
            .all(|(other, object)| other == index || !object.rect.intersects(rect))
    }

    /// Ближайшая свободная позиция на плиточной сетке — автосдвиг (ТЗ §6.1).
    ///
    /// Кандидаты перебираются по возрастанию расстояния, затем по координатам,
    /// поэтому результат детерминирован.
    fn nearest_free(&self, index: usize, target: Rect) -> Option<Rect> {
        const RADIUS: i32 = 12;
        let mut candidates = Vec::new();
        for dy in -RADIUS..=RADIUS {
            for dx in -RADIUS..=RADIUS {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let x = target.x + dx * TILE_UNIT;
                let y = target.y + dy * TILE_UNIT;
                if x < 0 || y < 0 {
                    continue;
                }
                candidates.push((dx * dx + dy * dy, x, y));
            }
        }
        candidates.sort_unstable();
        candidates.into_iter().find_map(|(_, x, y)| {
            let candidate = Rect { x, y, ..target };
            self.is_free(index, &candidate).then_some(candidate)
        })
    }

    /// Группа, в чьи границы попадает центр объекта.
    fn group_at(&self, rect: Rect) -> Option<String> {
        let (cx, cy) = rect.center();
        self.groups
            .iter()
            .find(|group| group.rect.contains_point(cx, cy))
            .map(|group| group.id.clone())
    }

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

    #[test]
    fn snap_rounds_to_8px_grid() {
        assert_eq!(snap_to_grid(0), 0);
        assert_eq!(snap_to_grid(6), 8);
        assert_eq!(snap_to_grid(14), 16);
        assert_eq!(snap_to_grid(96), 96);
        assert_eq!(snap_to_grid(105), 104);
        assert_eq!(snap_to_grid(208), 208);
    }

    #[test]
    fn default_layout_survives_its_own_coordinates() {
        for object in &Screen::default_screen().objects {
            assert_eq!(snap_to_grid(object.rect.x), object.rect.x, "{}", object.id);
            assert_eq!(snap_to_grid(object.rect.y), object.rect.y, "{}", object.id);
        }
    }

    #[test]
    fn free_move_keeps_snapped_position() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        let rect = screen.move_object("weather", 400, 100).expect("move");
        assert_eq!(
            rect,
            Rect {
                x: 400,
                y: 104,
                w: 176,
                h: 88
            }
        );
        assert_eq!(screen.validate(), Ok(()));
    }

    #[test]
    fn move_onto_occupied_slot_shifts_to_nearest_free() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        let rect = screen.move_object("weather", 16, 96).expect("move");
        assert_eq!(
            rect,
            Rect {
                x: 16,
                y: 8,
                w: 176,
                h: 88
            }
        );
        assert_eq!(screen.validate(), Ok(()));
    }

    #[test]
    fn move_changes_group_membership() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");

        let outside = screen.move_object("excel", 448, 208).expect("move out");
        assert_eq!((outside.x, outside.y), (448, 208));
        assert_eq!(screen.objects[4].group_id, None);
        assert_eq!(screen.validate(), Ok(()));

        let inside = screen.move_object("excel", 216, 208).expect("move back");
        assert_eq!((inside.x, inside.y), (216, 208));
        assert_eq!(screen.objects[4].group_id.as_deref(), Some("office"));
        assert_eq!(screen.validate(), Ok(()));
    }

    #[test]
    fn move_unknown_object_fails() {
        let mut screen = Screen::default_screen();
        assert_eq!(
            screen.move_object("nope", 0, 0),
            Err(MoveError::NotFound("nope".into()))
        );
    }

    #[test]
    fn preview_does_not_change_layout() {
        let screen = Screen::default_screen();
        let before = screen.clone();
        let rect = screen.preview_move("mail", 400, 400).expect("preview");
        assert_eq!((rect.x, rect.y), (400, 400));
        assert_eq!(screen, before);
    }

    #[test]
    fn moved_layout_round_trips_through_json() {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        screen.move_object("word", 600, 400).expect("move");
        let json = layout.to_json_pretty();
        let parsed: Layout = serde_json::from_str(&json).expect("parse");
        assert_eq!(layout, parsed);
    }
}
