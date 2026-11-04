//! Действия оболочки из трея и от второго экземпляра приложения.
//!
//! Обработчик ставится на UI-потоке и держит слабые ссылки на окна; команды из
//! чужих потоков приходят через [`post`] и попадают в цикл событий Slint.

/// Что просит сделать пользователь.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    /// Показать панель приложений (второй экземпляр просит показать оболочку).
    ShowPanel,
    /// Показать или скрыть панель (двойной клик по иконке трея).
    TogglePanels,
    /// Показать или скрыть верхнюю панель статуса.
    ToggleStatus,
    /// Показать или скрыть нижнюю панель сервисов.
    ToggleServices,
    /// Свернуть все панели (Ctrl+Alt+Esc).
    HideAll,
    /// На рабочую область бросили файлы или ярлыки (координаты — физические px).
    Drop {
        x: i32,
        y: i32,
        paths: Vec<std::path::PathBuf>,
    },
    /// Отменить последнее перемещение плитки.
    Undo,
    /// Повторить отменённое перемещение.
    Redo,
    /// Открыть каталог логов.
    OpenLogs,
    /// Завершить работу.
    Quit,
}

/// Обработчик действий на UI-потоке.
type Handler = Box<dyn Fn(Action)>;

thread_local! {
    static HANDLER: std::cell::RefCell<Option<Handler>> = const { std::cell::RefCell::new(None) };
}

/// Ставит обработчик действий; вызывается один раз при старте UI.
pub(crate) fn set_handler(handler: impl Fn(Action) + 'static) {
    HANDLER.with(|slot| *slot.borrow_mut() = Some(Box::new(handler)));
}

/// Выполняет действие в текущем (UI) потоке.
pub(crate) fn dispatch(action: Action) {
    HANDLER.with(|slot| {
        if let Some(handler) = slot.borrow().as_ref() {
            handler(action);
        }
    });
}

/// Перекидывает действие из другого потока в цикл событий Slint.
pub(crate) fn post(action: Action) {
    let _ = slint::invoke_from_event_loop(move || dispatch(action));
}
