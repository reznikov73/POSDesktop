//! История перемещений плиток: откат и повтор (ТЗ §6.3, A10).
//!
//! Хранит снимки раскладки, а не отдельные команды: раскладка маленькая, а
//! снимок гарантирует, что откат вернёт ровно то состояние, что было на экране.

use crate::layout::Layout;
use std::collections::VecDeque;

/// Сколько шагов отката хранится по ТЗ.
pub const HISTORY_LIMIT: usize = 20;

/// Стек отката и повтора.
#[derive(Debug, Clone)]
pub struct History {
    past: VecDeque<Layout>,
    future: Vec<Layout>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(HISTORY_LIMIT)
    }
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            past: VecDeque::new(),
            future: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Запоминает состояние перед изменением.
    pub fn record(&mut self, layout: &Layout) {
        if self.past.len() == self.limit {
            self.past.pop_front();
        }
        self.past.push_back(layout.clone());
        // Новое действие обнуляет ветку повтора.
        self.future.clear();
    }

    /// Откат: возвращает предыдущее состояние, текущее уходит в повтор.
    pub fn undo(&mut self, current: &Layout) -> Option<Layout> {
        let previous = self.past.pop_back()?;
        self.future.push(current.clone());
        Some(previous)
    }

    /// Повтор отменённого действия.
    pub fn redo(&mut self, current: &Layout) -> Option<Layout> {
        let next = self.future.pop()?;
        if self.past.len() == self.limit {
            self.past.pop_front();
        }
        self.past.push_back(current.clone());
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Забывает историю — например, после сброса раскладки.
    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_with_x(x: i32) -> Layout {
        let mut layout = Layout::default_primary();
        let screen = layout.monitors.get_mut("primary").expect("screen");
        screen.objects[0].rect.x = x;
        layout
    }

    fn x_of(layout: &Layout) -> i32 {
        layout.monitors["primary"].objects[0].rect.x
    }

    #[test]
    fn undo_returns_previous_state() {
        let mut history = History::default();
        let first = layout_with_x(16);
        let second = layout_with_x(64);

        history.record(&first);
        let restored = history.undo(&second).expect("undo");
        assert_eq!(x_of(&restored), 16);
        assert!(history.can_redo());

        let redone = history.redo(&restored).expect("redo");
        assert_eq!(x_of(&redone), 64);
        assert!(!history.can_redo());
    }

    #[test]
    fn empty_history_gives_none() {
        let mut history = History::default();
        assert_eq!(history.undo(&Layout::default_primary()), None);
        assert_eq!(history.redo(&Layout::default_primary()), None);
    }

    #[test]
    fn new_action_drops_redo_branch() {
        let mut history = History::default();
        history.record(&layout_with_x(16));
        let undone = history.undo(&layout_with_x(64)).expect("undo");
        assert!(history.can_redo());

        history.record(&undone);
        assert!(!history.can_redo());
    }

    #[test]
    fn limit_keeps_last_twenty() {
        let mut history = History::default();
        for step in 0..25 {
            history.record(&layout_with_x(step * 8));
        }
        let mut current = layout_with_x(999);
        let mut steps = 0;
        while let Some(previous) = history.undo(&current) {
            current = previous;
            steps += 1;
        }
        assert_eq!(steps, HISTORY_LIMIT);
        assert_eq!(x_of(&current), (25 - HISTORY_LIMIT as i32) * 8);
    }

    #[test]
    fn clear_forgets_everything() {
        let mut history = History::default();
        history.record(&layout_with_x(16));
        history.clear();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }
}
