# POS Desktop

Кастомизируемая оболочка рабочего стола Windows: четыре панели по краям экрана, рабочая область с плитками, виджет почты и ИИ-ассистент.

**Статус:** каркас проекта (M0.5). UI ещё не подключён — сначала проверяется механика окон и триггеров (D0).

- Техническое задание: [docs/ТЗ.md](docs/ТЗ.md)
- Репозиторий: https://github.com/reznikov73/POSDesktop

## Структура

```
crates/core           модель раскладки, валидация
crates/shell          логика триггеров у краёв экрана
crates/ai             реестр инструментов ИИ и политика подтверждений
crates/integrations   источники данных (почта)
crates/ui             UI-слой на Slint
crates/app            бинарник posdesktop: окно + CLI
apps/d0-spike         замер триггеров на реальном экране
ui/app.slint          экраны интерфейса
```

## Запуск

```powershell
cargo run -p posdesktop                  # окно приложения
cargo run -p posdesktop -- --overlay     # оверлей: поверх всех, без фокуса
cargo run -p posdesktop -- --demo        # CLI: модели данных
cargo run -p posdesktop-d0-spike         # триггеры краёв экрана
```

В VS Code: `F5` → «Debug POS Desktop» или «Debug POS Desktop (overlay)».
Оверлейное окно не берёт фокус, поэтому `Alt+F4` его не закрывает —
останавливайте через Stop в отладчике или `Ctrl+C` в терминале.

Логи: `logs\app.log` в корне проекта (ротация 5 МБ × 5), путь можно
переопределить переменной `POSDESKTOP_LOG_DIR`. Открыть папку:
`posdesktop.exe --logs`. Уровень: `$env:POSDESKTOP_LOG = 'debug'`.

## Сборка

```powershell
cargo test --workspace
cargo build --release -p posdesktop
cargo build --release -p posdesktop-ui-app
.\scripts\build.ps1          # fmt + clippy + tests + release + dist
.\scripts\release.ps1        # + версия, MSI, коммит, push
```

Замеры D0 (память, CPU, размер): [docs/D0-measurements.md](docs/D0-measurements.md).

Тема по умолчанию — светлая, палитра взята из макета прототипа:
фон `#fbfcfc`, поверхности `#f1f4f5`, текст `#17181a`, акцент `#0c7f68`.

## Лицензия

MIT.
