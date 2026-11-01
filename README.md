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
crates/app            CLI-бинарник posdesktop
apps/d0-spike         замер триггеров на реальном экране
```

## Сборка

```powershell
cargo test --workspace
cargo build --release -p posdesktop
.\scripts\build.ps1          # fmt + clippy + tests + release + dist
.\scripts\release.ps1        # + версия, MSI, коммит, push
```

## Лицензия

MIT.
