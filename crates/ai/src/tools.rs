//! Инструменты ИИ: классы действий и правила подтверждений из ТЗ §8.

/// Класс действия инструмента.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolClass {
    /// Чтение данных.
    Read,
    /// Запуск известного процесса, не меняющий данные.
    Action,
    /// Изменение данных без удаления.
    Write,
    /// Необратимая операция.
    Destructive,
    /// Управление самой оболочкой.
    Ui,
}

/// Требуемое подтверждение.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    None,
    Single,
    Double,
}

/// Итоговая политика для вызова.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub confirmation: Confirmation,
    pub allowed: bool,
}

/// Контекст вызова.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    /// Приложение выбрано из каталога, а не по произвольному пути.
    pub from_catalog: bool,
    /// Объект находится в рабочей папке пользователя.
    pub in_work_folder: bool,
}

/// Реестр инструментов: имя и класс.
pub const TOOLS: &[(&str, ToolClass)] = &[
    ("mail.count", ToolClass::Read),
    ("mail.list", ToolClass::Read),
    ("mail.summary", ToolClass::Read),
    ("services.status", ToolClass::Read),
    ("apps.list", ToolClass::Read),
    ("files.search", ToolClass::Read),
    ("apps.launch", ToolClass::Action),
    ("files.open", ToolClass::Write),
    ("files.move", ToolClass::Write),
    ("files.rename", ToolClass::Write),
    ("services.start", ToolClass::Write),
    ("services.stop", ToolClass::Write),
    ("files.delete", ToolClass::Destructive),
    ("mail.delete", ToolClass::Destructive),
    ("script.run", ToolClass::Destructive),
    ("panels.open", ToolClass::Ui),
    ("panels.close", ToolClass::Ui),
    ("desktop.move_tile", ToolClass::Ui),
];

pub fn class_of(name: &str) -> Option<ToolClass> {
    TOOLS
        .iter()
        .find(|(tool, _)| *tool == name)
        .map(|(_, class)| *class)
}

/// Политика подтверждений по ТЗ §8.2–8.3.
pub fn policy_for(name: &str, ctx: Context) -> Policy {
    let Some(class) = class_of(name) else {
        // Неизвестный инструмент не выполняется и требует подтверждения.
        return Policy {
            confirmation: Confirmation::Double,
            allowed: false,
        };
    };

    match class {
        ToolClass::Read => {
            if name == "files.search" && !ctx.in_work_folder {
                Policy {
                    confirmation: Confirmation::Single,
                    allowed: true,
                }
            } else {
                Policy {
                    confirmation: Confirmation::None,
                    allowed: true,
                }
            }
        }
        ToolClass::Action => {
            if name == "apps.launch" && !ctx.from_catalog {
                Policy {
                    confirmation: Confirmation::Single,
                    allowed: true,
                }
            } else {
                Policy {
                    confirmation: Confirmation::None,
                    allowed: true,
                }
            }
        }
        ToolClass::Write => Policy {
            confirmation: Confirmation::Single,
            allowed: true,
        },
        ToolClass::Destructive => {
            if name == "files.delete" && !ctx.in_work_folder {
                Policy {
                    confirmation: Confirmation::Double,
                    allowed: false,
                }
            } else {
                Policy {
                    confirmation: Confirmation::Double,
                    allowed: true,
                }
            }
        }
        ToolClass::Ui => Policy {
            confirmation: Confirmation::None,
            allowed: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(from_catalog: bool, in_work_folder: bool) -> Context {
        Context {
            from_catalog,
            in_work_folder,
        }
    }

    #[test]
    fn unknown_tool_is_blocked() {
        let policy = policy_for("files.wipe_disk", ctx(false, true));
        assert!(!policy.allowed);
        assert_eq!(policy.confirmation, Confirmation::Double);
    }

    #[test]
    fn read_tool_needs_no_confirmation() {
        let policy = policy_for("mail.count", ctx(false, false));
        assert!(policy.allowed);
        assert_eq!(policy.confirmation, Confirmation::None);
    }

    #[test]
    fn search_outside_work_folder_requires_confirmation() {
        let policy = policy_for("files.search", ctx(false, false));
        assert!(policy.allowed);
        assert_eq!(policy.confirmation, Confirmation::Single);
    }

    #[test]
    fn search_inside_work_folder_is_free() {
        let policy = policy_for("files.search", ctx(false, true));
        assert_eq!(policy.confirmation, Confirmation::None);
    }

    #[test]
    fn launch_from_catalog_is_free() {
        let policy = policy_for("apps.launch", ctx(true, false));
        assert_eq!(policy.confirmation, Confirmation::None);
    }

    #[test]
    fn launch_arbitrary_path_requires_confirmation() {
        let policy = policy_for("apps.launch", ctx(false, false));
        assert_eq!(policy.confirmation, Confirmation::Single);
    }

    #[test]
    fn delete_inside_work_folder_is_double_confirmed() {
        let policy = policy_for("files.delete", ctx(false, true));
        assert!(policy.allowed);
        assert_eq!(policy.confirmation, Confirmation::Double);
    }

    #[test]
    fn delete_outside_work_folder_is_forbidden() {
        let policy = policy_for("files.delete", ctx(false, false));
        assert!(!policy.allowed);
    }

    #[test]
    fn script_run_requires_double_confirmation() {
        let policy = policy_for("script.run", ctx(false, true));
        assert!(policy.allowed);
        assert_eq!(policy.confirmation, Confirmation::Double);
    }

    #[test]
    fn ui_tools_are_free() {
        let policy = policy_for("panels.open", ctx(false, false));
        assert!(policy.allowed);
        assert_eq!(policy.confirmation, Confirmation::None);
    }
}
