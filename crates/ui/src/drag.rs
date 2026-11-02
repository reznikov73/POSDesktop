//! Перетаскивание плиток рабочей области: привязка к сетке, автосдвиг при
//! пересечении и сохранение в `layout.json` (ТЗ §6.1, §6.3, §6.5).
//!
//! Нажатие сообщает Slint (`tile-pressed`), а движение и отпускание кнопки
//! опрашиваются через Win32: окно рабочей области создаётся с
//! `WS_EX_NOACTIVATE`, и полагаться на поток событий указателя от winit нельзя.

use posdesktop_core::{save, History, Layout, Object, Rect};
use posdesktop_shell::win32;
use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crate::{Tile, WorkspaceWindow};

/// Период опроса курсора во время перетаскивания — кадр 60 Гц (ТЗ §12, N7).
const FRAME_MS: u64 = 16;
/// Период опроса в покое: перетаскивания нет, кадры не нужны.
const IDLE_MS: u64 = 1000;
/// Отступ окна за габаритами плиток, логические px.
const WINDOW_MARGIN: i32 = 16;
/// Минимальный размер окна рабочей области, логические px.
const MIN_WINDOW: (i32, i32) = (320, 240);

/// Рабочая область: раскладка — единственный источник истины (ТЗ §4.2).
pub(crate) struct Workspace {
    layout: Layout,
    path: PathBuf,
    /// Ключ монитора, с которым работает окно.
    screen_key: String,
    model: Rc<VecModel<Tile>>,
    drag: Option<Drag>,
    /// Когда закончилось последнее перетаскивание — чтобы клик после него не запускал приложение.
    drag_ended: Option<std::time::Instant>,
    /// Откат и повтор перемещений (ТЗ §6.3).
    history: History,
    /// Таймер кадров; в покое тикает редко.
    timer: Timer,
}

/// Активное перетаскивание плитки.
struct Drag {
    index: usize,
    id: String,
    /// Смещение точки захвата от левого верхнего угла плитки, логические px.
    grab: (f32, f32),
    /// Прямоугольник на момент нажатия — для отката и габаритов окна.
    start: Rect,
}

/// Готовит модель плиток и обработчики перетаскивания.
pub(crate) fn install(
    ui: &WorkspaceWindow,
    layout: Layout,
    path: PathBuf,
) -> Rc<RefCell<Workspace>> {
    let screen_key = layout
        .monitors
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "primary".to_string());
    let model = Rc::new(VecModel::from(screen_tiles(&layout, &screen_key)));
    ui.set_tiles(ModelRc::from(model.clone()));

    let state = Rc::new(RefCell::new(Workspace {
        layout,
        path,
        screen_key,
        model,
        drag: None,
        drag_ended: None,
        history: History::default(),
        timer: Timer::default(),
    }));

    let timer_state = Rc::downgrade(&state);
    let timer_ui = ui.as_weak();
    state.borrow().timer.start(
        TimerMode::Repeated,
        Duration::from_millis(IDLE_MS),
        move || {
            if let (Some(state), Some(ui)) = (timer_state.upgrade(), timer_ui.upgrade()) {
                frame(&state, &ui);
            }
        },
    );

    let press_state = Rc::downgrade(&state);
    let press_ui = ui.as_weak();
    ui.on_tile_pressed(move |index| {
        if let (Some(state), Some(ui)) = (press_state.upgrade(), press_ui.upgrade()) {
            begin(&state, &ui, index);
        }
    });

    let click_state = Rc::downgrade(&state);
    ui.on_tile_clicked(move |index| {
        if let Some(state) = click_state.upgrade() {
            state.borrow().activate(index);
        }
    });

    state
}

impl Workspace {
    /// Габариты окна по габаритам плиток, логические px.
    pub(crate) fn window_size(&self) -> (i32, i32) {
        let mut width = MIN_WINDOW.0;
        let mut height = MIN_WINDOW.1;
        if let Some(screen) = self.layout.monitors.get(&self.screen_key) {
            for object in &screen.objects {
                width = width.max(object.rect.right() + WINDOW_MARGIN);
                height = height.max(object.rect.bottom() + WINDOW_MARGIN);
            }
        }
        (width, height)
    }

    /// Прямоугольники плиток в физических пикселях — для `WM_NCHITTEST`.
    pub(crate) fn physical_rects(&self, scale: f32) -> Vec<(i32, i32, i32, i32)> {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let Some(screen) = self.layout.monitors.get(&self.screen_key) else {
            return Vec::new();
        };
        screen
            .objects
            .iter()
            .map(|object| {
                let rect = object.rect;
                (
                    (rect.x as f32 * scale).round() as i32,
                    (rect.y as f32 * scale).round() as i32,
                    (rect.w as f32 * scale).round() as i32,
                    (rect.h as f32 * scale).round() as i32,
                )
            })
            .collect()
    }

    fn object(&self, index: usize) -> Option<(String, Rect)> {
        let screen = self.layout.monitors.get(&self.screen_key)?;
        let object = screen.objects.get(index)?;
        Some((object.id.clone(), object.rect))
    }

    /// Предпросмотр привязки: где окажется плитка, если отпустить сейчас.
    fn ghost_rect(&self, id: &str, x: i32, y: i32) -> Option<Rect> {
        let screen = self.layout.monitors.get(&self.screen_key)?;
        screen.preview_move(id, x, y).ok()
    }

    fn persist(&self) {
        match save(&self.path, &self.layout) {
            Ok(()) => log::info!("layout.json сохранён: {}", self.path.display()),
            Err(err) => log::error!("не удалось сохранить layout.json: {err}"),
        }
    }

    /// Клик по плитке: запуск приложения из каталога «Пуск».
    ///
    /// Сразу после перетаскивания клик игнорируется — иначе отпускание плитки
    /// запускало бы приложение.
    pub(crate) fn activate(&self, index: i32) {
        if self
            .drag_ended
            .is_some_and(|ended| ended.elapsed() < Duration::from_millis(300))
        {
            return;
        }
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(screen) = self.layout.monitors.get(&self.screen_key) else {
            return;
        };
        let Some(object) = screen.objects.get(index) else {
            return;
        };

        if let Some(app) = &object.app {
            launch_app(app);
        } else if let Some(widget) = &object.widget {
            log::info!("виджет {widget}: действия пока нет");
        }
    }

    /// Откат последнего перемещения.
    pub(crate) fn undo(&mut self, ui: &WorkspaceWindow) -> bool {
        let Some(previous) = self.history.undo(&self.layout) else {
            log::info!("отменять нечего");
            return false;
        };
        self.layout = previous;
        self.apply(ui);
        log::info!("перемещение отменено");
        true
    }

    /// Повтор отменённого перемещения.
    pub(crate) fn redo(&mut self, ui: &WorkspaceWindow) -> bool {
        let Some(next) = self.history.redo(&self.layout) else {
            log::info!("повторять нечего");
            return false;
        };
        self.layout = next;
        self.apply(ui);
        log::info!("перемещение повторено");
        true
    }

    /// Приводит окно и модель к текущей раскладке.
    fn apply(&mut self, ui: &WorkspaceWindow) {
        let tiles = screen_tiles(&self.layout, &self.screen_key);
        self.model.set_vec(tiles);
        self.persist();
        self.refresh(ui);
    }

    /// Пересчитывает окно и зоны клик-сквозь после перетаскивания.
    fn refresh(&self, ui: &WorkspaceWindow) {
        let (width, height) = self.window_size();
        ui.set_ws_width(width as f32);
        ui.set_ws_height(height as f32);
        crate::update_pass_rects(&self.physical_rects(ui.window().scale_factor()));
    }
}

/// Начало перетаскивания: точка захвата берётся из курсора.
fn begin(state: &Rc<RefCell<Workspace>>, ui: &WorkspaceWindow, index: i32) {
    let mut this = state.borrow_mut();
    if this.drag.is_some() {
        return;
    }
    let Ok(index) = usize::try_from(index) else {
        return;
    };
    let Some((id, rect)) = this.object(index) else {
        log::warn!("перетаскивание: плитка #{index} не найдена в раскладке");
        return;
    };
    let Some((cursor_x, cursor_y)) = win32::cursor_pos() else {
        log::warn!("перетаскивание: курсор недоступен");
        return;
    };
    let scale = ui.window().scale_factor().max(0.5);
    let local_x = cursor_x as f32 / scale;
    let local_y = cursor_y as f32 / scale;
    let grab = (
        (local_x - rect.x as f32).clamp(0.0, rect.w as f32),
        (local_y - rect.y as f32).clamp(0.0, rect.h as f32),
    );

    this.drag = Some(Drag {
        index,
        id: id.clone(),
        grab,
        start: rect,
    });
    ui.set_drag_index(index as i32);
    ui.set_ghost_visible(false);
    this.timer.set_interval(Duration::from_millis(FRAME_MS));
    log::info!(
        "плитка {id}: перетаскивание начато, захват ({:.0}, {:.0})",
        grab.0,
        grab.1
    );
}

/// Кадр перетаскивания: двигаем плитку или завершаем операцию.
fn frame(state: &Rc<RefCell<Workspace>>, ui: &WorkspaceWindow) {
    let mut this = state.borrow_mut();
    let Some((index, grab, start)) = this
        .drag
        .as_ref()
        .map(|drag| (drag.index, drag.grab, drag.start))
    else {
        return;
    };
    let Some((cursor_x, cursor_y)) = win32::cursor_pos() else {
        return;
    };
    let scale = ui.window().scale_factor().max(0.5);
    let local_x = cursor_x as f32 / scale;
    let local_y = cursor_y as f32 / scale;

    if win32::left_button_down() {
        this.preview(ui, index, start, grab, local_x, local_y);
    } else {
        this.finish(ui, index, start, grab);
    }
}

impl Workspace {
    /// Плитка следует за курсором, призрак показывает место после привязки.
    fn preview(
        &mut self,
        ui: &WorkspaceWindow,
        index: usize,
        start: Rect,
        grab: (f32, f32),
        cursor_x: f32,
        cursor_y: f32,
    ) {
        let x = (cursor_x - grab.0).max(0.0).round() as i32;
        let y = (cursor_y - grab.1).max(0.0).round() as i32;

        if let Some(mut tile) = self.model.row_data(index) {
            if tile.x != x as f32 || tile.y != y as f32 {
                tile.x = x as f32;
                tile.y = y as f32;
                self.model.set_row_data(index, tile);
            }
        }

        if let Some(drag) = self.drag.as_ref() {
            if let Some(rect) = self.ghost_rect(&drag.id, x, y) {
                ui.set_ghost_x(rect.x as f32);
                ui.set_ghost_y(rect.y as f32);
                ui.set_ghost_w(rect.w as f32);
                ui.set_ghost_h(rect.h as f32);
                ui.set_ghost_visible(true);
            }
        }

        // Плитку можно увести за габариты окна — тогда окно подрастает.
        let (width, height) = self.window_size();
        let needed_width = x + start.w + WINDOW_MARGIN;
        let needed_height = y + start.h + WINDOW_MARGIN;
        if needed_width > width || needed_height > height {
            ui.set_ws_width(width.max(needed_width) as f32);
            ui.set_ws_height(height.max(needed_height) as f32);
        }
    }

    /// Отпускание: привязка, автосдвиг, группа и запись в `layout.json`.
    fn finish(&mut self, ui: &WorkspaceWindow, index: usize, start: Rect, grab: (f32, f32)) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        ui.set_drag_index(-1);
        ui.set_ghost_visible(false);
        self.timer.set_interval(Duration::from_millis(IDLE_MS));

        let Some(mut tile) = self.model.row_data(index) else {
            return;
        };
        // Позицию берём по курсору: модель могла отстать на кадр.
        let before = self.layout.clone();
        let (x, y) = match win32::cursor_pos() {
            Some((cursor_x, cursor_y)) => {
                let scale = ui.window().scale_factor().max(0.5);
                (
                    (cursor_x as f32 / scale - grab.0).max(0.0).round() as i32,
                    (cursor_y as f32 / scale - grab.1).max(0.0).round() as i32,
                )
            }
            None => (tile.x as i32, tile.y as i32),
        };
        // Клик без движения — это не перемещение: плитка остаётся на месте,
        // а обработчик клика запустит приложение.
        if (x - start.x).abs() <= 3 && (y - start.y).abs() <= 3 {
            tile.x = start.x as f32;
            tile.y = start.y as f32;
            self.model.set_row_data(index, tile);
            return;
        }

        let moved = match self.layout.monitors.get_mut(&self.screen_key) {
            Some(screen) => screen.move_object(&drag.id, x, y),
            None => return,
        };

        match moved {
            Ok(rect) => {
                tile.x = rect.x as f32;
                tile.y = rect.y as f32;
                self.model.set_row_data(index, tile);
                log::info!("плитка {} встала в ({}, {})", drag.id, rect.x, rect.y);
                if rect != drag.start {
                    self.history.record(&before);
                }
                self.persist();
            }
            Err(err) => {
                tile.x = start.x as f32;
                tile.y = start.y as f32;
                self.model.set_row_data(index, tile);
                log::warn!("плитка {} осталась на месте: {err}", drag.id);
            }
        }

        self.refresh(ui);
        self.drag_ended = Some(std::time::Instant::now());
    }
}

/// Запускает приложение по названию: ищем ярлык в меню «Пуск».
fn launch_app(name: &str) {
    let Some(path) = posdesktop_shell::icons::find_shortcut(name) else {
        log::warn!("запуск {name}: ярлык в меню «Пуск» не найден");
        return;
    };
    if posdesktop_shell::launch::open(&path) {
        log::info!("запуск {name}: {}", path.display());
    } else {
        log::warn!("запуск {name}: ShellExecute не открыл {}", path.display());
    }
}

/// Плитки одного монитора для модели Slint.
pub(crate) fn screen_tiles(layout: &Layout, screen_key: &str) -> Vec<Tile> {
    layout
        .monitors
        .get(screen_key)
        .map(|screen| screen.objects.iter().map(tile_of).collect())
        .unwrap_or_default()
}

/// Плитка Slint из объекта раскладки.
pub(crate) fn tile_of(object: &Object) -> Tile {
    let label = object.label();
    let icon = posdesktop_shell::icons::icon_for_app(label, 32);
    let has_icon = icon.is_some();
    let image = icon
        .map(|pixels| {
            slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
                &pixels.rgba,
                pixels.width,
                pixels.height,
            ))
        })
        .unwrap_or_default();
    Tile {
        x: object.rect.x as f32,
        y: object.rect.y as f32,
        w: object.rect.w as f32,
        h: object.rect.h as f32,
        label: label.into(),
        badge: badge_of(label).into(),
        icon: image,
        has_icon,
    }
}

/// Монограмма плитки: первая буква названия, пока нет иконок из Shell.
fn badge_of(label: &str) -> String {
    label
        .chars()
        .next()
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_default()
}
