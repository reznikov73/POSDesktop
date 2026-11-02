//! Счётчики почты из базы Soroka (ТЗ §7.2).
//!
//! База открывается только на чтение через системную `winsqlite3.dll` — новых
//! зависимостей не нужно. Любая ошибка (нет файла, чужая схема, занятая база)
//! возвращает `None`: оболочка покажет прочерк, а не упадёт.

use crate::MailSource;
use std::path::{Path, PathBuf};

/// Снимок счётчиков почты.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MailSummary {
    /// Непрочитанные по папкам, только непустые.
    pub unread: Vec<(String, u32)>,
    /// Всего писем в базе.
    pub total: u32,
    /// Время последней синхронизации Soroka (unix-секунды).
    pub last_sync: Option<i64>,
}

impl MailSummary {
    pub fn unread_total(&self) -> u32 {
        self.unread.iter().map(|(_, count)| *count).sum()
    }
}

impl MailSource for MailSummary {
    fn unread_total(&self) -> u32 {
        MailSummary::unread_total(self)
    }

    fn unread_by_box(&self) -> Vec<(String, u32)> {
        self.unread.clone()
    }
}

/// Читатель базы Soroka.
#[derive(Debug, Clone)]
pub struct SorokaMail {
    path: PathBuf,
}

impl SorokaMail {
    /// Открывает базу, если она есть и схема узнаваема.
    pub fn open() -> Option<Self> {
        let path = database_path()?;
        if !path.is_file() {
            return None;
        }
        let mail = Self { path };
        // Проверяем схему дешёвым запросом.
        mail.read().map(|_| mail)
    }

    /// Путь к базе Soroka.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Читает счётчики.
    pub fn read(&self) -> Option<MailSummary> {
        imp::read(&self.path)
    }
}

/// Путь к базе Soroka: `%APPDATA%\Soroka\soroka.db` с запасным профилем.
pub fn database_path() -> Option<PathBuf> {
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| {
        std::env::var_os("USERPROFILE")
            .map(|profile| PathBuf::from(profile).join("AppData").join("Roaming"))
    })?;
    Some(roaming.join("Soroka").join("soroka.db"))
}

#[cfg(windows)]
mod imp {
    use super::MailSummary;
    use std::ffi::{c_char, c_int, c_uchar, c_void, CStr, CString};
    use std::path::Path;
    use std::ptr;

    const SQLITE_OK: c_int = 0;
    const SQLITE_ROW: c_int = 100;
    const SQLITE_OPEN_READONLY: c_int = 0x0000_0001;

    #[link(name = "winsqlite3")]
    extern "system" {
        fn sqlite3_open_v2(
            filename: *const c_char,
            db: *mut *mut c_void,
            flags: c_int,
            vfs: *const c_char,
        ) -> c_int;
        fn sqlite3_close(db: *mut c_void) -> c_int;
        fn sqlite3_prepare_v2(
            db: *mut c_void,
            sql: *const c_char,
            bytes: c_int,
            stmt: *mut *mut c_void,
            tail: *mut *const c_char,
        ) -> c_int;
        fn sqlite3_step(stmt: *mut c_void) -> c_int;
        fn sqlite3_column_text(stmt: *mut c_void, column: c_int) -> *const c_uchar;
        fn sqlite3_column_int(stmt: *mut c_void, column: c_int) -> c_int;
        fn sqlite3_column_int64(stmt: *mut c_void, column: c_int) -> i64;
        fn sqlite3_finalize(stmt: *mut c_void) -> c_int;
    }

    struct Statement(*mut c_void);

    impl Drop for Statement {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { sqlite3_finalize(self.0) };
            }
        }
    }

    struct Database(*mut c_void);

    impl Drop for Database {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { sqlite3_close(self.0) };
            }
        }
    }

    fn open(path: &Path) -> Option<Database> {
        let path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
        let mut db = ptr::null_mut();
        let code =
            unsafe { sqlite3_open_v2(path.as_ptr(), &mut db, SQLITE_OPEN_READONLY, ptr::null()) };
        if code != SQLITE_OK {
            return None;
        }
        Some(Database(db))
    }

    fn prepare(db: &Database, sql: &str) -> Option<Statement> {
        let sql = CString::new(sql).ok()?;
        let mut stmt = ptr::null_mut();
        let code =
            unsafe { sqlite3_prepare_v2(db.0, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if code != SQLITE_OK || stmt.is_null() {
            return None;
        }
        Some(Statement(stmt))
    }

    fn step(stmt: &Statement) -> c_int {
        unsafe { sqlite3_step(stmt.0) }
    }

    fn column_text(stmt: &Statement, column: c_int) -> Option<String> {
        let text = unsafe { sqlite3_column_text(stmt.0, column) };
        if text.is_null() {
            return None;
        }
        Some(
            unsafe { CStr::from_ptr(text as *const c_char) }
                .to_string_lossy()
                .into_owned(),
        )
    }

    fn column_int(stmt: &Statement, column: c_int) -> i32 {
        unsafe { sqlite3_column_int(stmt.0, column) }
    }

    fn column_int64(stmt: &Statement, column: c_int) -> i64 {
        unsafe { sqlite3_column_int64(stmt.0, column) }
    }

    pub(super) fn read(path: &Path) -> Option<MailSummary> {
        let db = open(path)?;
        let mut summary = MailSummary::default();

        {
            let stmt = prepare(
                &db,
                "select folder, unseen from folder_stats order by folder",
            )?;
            while step(&stmt) == SQLITE_ROW {
                let folder = column_text(&stmt, 0).unwrap_or_default();
                let unseen = column_int(&stmt, 1).max(0) as u32;
                if unseen > 0 {
                    summary.unread.push((folder, unseen));
                }
            }
        }

        if let Some(stmt) = prepare(&db, "select key, value from meta") {
            while step(&stmt) == SQLITE_ROW {
                let Some(key) = column_text(&stmt, 0) else {
                    continue;
                };
                match key.as_str() {
                    "total" => {
                        summary.total = column_text(&stmt, 1)
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(0);
                    }
                    "last_sync" => {
                        summary.last_sync =
                            column_text(&stmt, 1).and_then(|value| value.parse().ok());
                    }
                    _ => {}
                }
                let _ = column_int64(&stmt, 1);
            }
        }

        Some(summary)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::MailSummary;
    use std::path::Path;

    pub(super) fn read(_path: &Path) -> Option<MailSummary> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_unread() {
        let summary = MailSummary {
            unread: vec![("work".into(), 4), ("projects".into(), 1)],
            total: 36,
            last_sync: Some(1_793_477_391),
        };
        assert_eq!(summary.unread_total(), 5);
        assert_eq!(summary.unread_by_box().len(), 2);
    }

    #[test]
    fn missing_database_gives_none() {
        let path = PathBuf::from("Z:\\нет-такой-папки\\soroka.db");
        assert!(imp::read(&path).is_none());
    }

    #[test]
    fn real_database_reads_if_present() {
        // На машинах без Soroka тест просто не проверяет чтение.
        if let Some(mail) = SorokaMail::open() {
            let summary = mail.read().expect("счётчики читаются");
            assert!(summary.total >= summary.unread_total());
        }
    }
}
