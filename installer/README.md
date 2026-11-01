# Установщик (WiX)

MSI собирается через `cargo-wix`.

## Первичная настройка

```powershell
cargo install cargo-wix
cargo wix init        # создаёт wix\main.wxs и wix\License.rtf
```

`cargo wix init` генерирует шаблон под текущий `Cargo.toml`. После этого
`wix\main.wxs` — обычный файл проекта, его можно править: имя продукта,
UpgradeCode, ярлыки, AUMID.

## Сборка

```powershell
.\scripts\installer.ps1
```

Результат: `dist\POSDesktop-<version>-x64.msi`.

## Требования к MSI

| Параметр | Значение |
|---|---|
| Тип установки | per-user (`ALLUSERS=2`, `MSIINSTALLPERUSER=1`) — без UAC |
| UpgradeCode | фиксированный GUID на весь срок жизни продукта |
| Версия | синхронизируется `scripts/version.ps1` |
| Ярлыки | меню «Пуск» + AUMID-ярлык для Toast |
| Данные | `%APPDATA%` сохраняются при удалении |

WiX toolset (`candle.exe`, `light.exe`) ставится автоматически при первом
запуске `cargo wix`.
