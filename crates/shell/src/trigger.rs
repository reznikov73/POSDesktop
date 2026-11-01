//! Определение края экрана по позиции курсора (ТЗ §5.6).

use posdesktop_core::Edge;

/// Прямоугольник монитора в физических пикселях.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Параметры зон активации.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerConfig {
    /// Ширина полосы-триггера.
    pub zone_px: i32,
    /// Исключение углов (горячие зоны Windows).
    pub corner_px: i32,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            zone_px: 6,
            corner_px: 32,
        }
    }
}

/// Возвращает край, если курсор в зоне активации.
///
/// Углы исключены: там системные горячие зоны Windows.
pub fn edge_at(cursor: (i32, i32), screen: ScreenRect, cfg: &TriggerConfig) -> Option<Edge> {
    let (cx, cy) = cursor;

    if cx < screen.x || cy < screen.y || cx >= screen.x + screen.w || cy >= screen.y + screen.h {
        return None;
    }

    let left = cx - screen.x;
    let right = screen.x + screen.w - 1 - cx;
    let top = cy - screen.y;
    let bottom = screen.y + screen.h - 1 - cy;

    let near_side = left < cfg.corner_px || right < cfg.corner_px;
    let near_corner_y = top < cfg.corner_px || bottom < cfg.corner_px;
    if near_side && near_corner_y {
        return None;
    }

    let mut best: Option<(i32, Edge)> = None;
    for (distance, edge) in [
        (left, Edge::Left),
        (right, Edge::Right),
        (top, Edge::Top),
        (bottom, Edge::Bottom),
    ] {
        if distance <= cfg.zone_px && best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, edge));
        }
    }

    best.map(|(_, edge)| edge)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> ScreenRect {
        ScreenRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        }
    }

    #[test]
    fn center_is_inactive() {
        assert_eq!(
            edge_at((960, 540), screen(), &TriggerConfig::default()),
            None
        );
    }

    #[test]
    fn left_edge_is_active() {
        assert_eq!(
            edge_at((2, 540), screen(), &TriggerConfig::default()),
            Some(Edge::Left)
        );
    }

    #[test]
    fn right_edge_is_active() {
        assert_eq!(
            edge_at((1918, 540), screen(), &TriggerConfig::default()),
            Some(Edge::Right)
        );
    }

    #[test]
    fn top_edge_is_active() {
        assert_eq!(
            edge_at((960, 3), screen(), &TriggerConfig::default()),
            Some(Edge::Top)
        );
    }

    #[test]
    fn bottom_edge_is_active() {
        assert_eq!(
            edge_at((960, 1078), screen(), &TriggerConfig::default()),
            Some(Edge::Bottom)
        );
    }

    #[test]
    fn corners_are_excluded() {
        let cfg = TriggerConfig::default();
        assert_eq!(edge_at((2, 2), screen(), &cfg), None);
        assert_eq!(edge_at((1918, 2), screen(), &cfg), None);
        assert_eq!(edge_at((2, 1078), screen(), &cfg), None);
        assert_eq!(edge_at((1918, 1078), screen(), &cfg), None);
    }

    #[test]
    fn outside_screen_is_inactive() {
        assert_eq!(
            edge_at((-1, 540), screen(), &TriggerConfig::default()),
            None
        );
        assert_eq!(
            edge_at((2000, 540), screen(), &TriggerConfig::default()),
            None
        );
    }

    #[test]
    fn secondary_monitor_offset_is_respected() {
        let secondary = ScreenRect {
            x: 1920,
            y: 0,
            w: 1280,
            h: 1024,
        };
        assert_eq!(
            edge_at((1921, 500), secondary, &TriggerConfig::default()),
            Some(Edge::Left)
        );
        assert_eq!(
            edge_at((1830, 500), secondary, &TriggerConfig::default()),
            None
        );
    }

    #[test]
    fn nearest_edge_wins() {
        // Ближе к левому краю (1 px), чем к верхнему (4 px).
        assert_eq!(
            edge_at((1, 40), screen(), &TriggerConfig::default()),
            Some(Edge::Left)
        );
    }
}
