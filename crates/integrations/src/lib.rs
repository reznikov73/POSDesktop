//! Интеграции: источники данных для виджетов.

/// Источник счётчиков почты. Реализации: Soroka IPC, Soroka SQLite, IMAP.
pub trait MailSource {
    fn unread_total(&self) -> u32;
    fn unread_by_box(&self) -> Vec<(String, u32)>;
}

/// Демонстрационный источник для каркаса и тестов.
#[derive(Debug, Clone, Default)]
pub struct DemoMail {
    pub boxes: Vec<(String, u32)>,
}

impl DemoMail {
    pub fn new(boxes: &[(&str, u32)]) -> Self {
        Self {
            boxes: boxes
                .iter()
                .map(|(name, count)| ((*name).to_string(), *count))
                .collect(),
        }
    }
}

impl MailSource for DemoMail {
    fn unread_total(&self) -> u32 {
        self.boxes.iter().map(|(_, count)| *count).sum()
    }

    fn unread_by_box(&self) -> Vec<(String, u32)> {
        self.boxes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_add_up() {
        let mail = DemoMail::new(&[("work", 5), ("projects", 7)]);
        assert_eq!(mail.unread_total(), 12);
        assert_eq!(mail.unread_by_box().len(), 2);
    }

    #[test]
    fn empty_source_has_zero() {
        let mail = DemoMail::default();
        assert_eq!(mail.unread_total(), 0);
    }
}
