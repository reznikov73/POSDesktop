# AGENTS.md

Проект: POS Desktop — оболочка рабочего стола Windows (Rust + Slint).
Канонический путь: `E:\Work\VSC\POSDesktop`.

**Сначала прочитай:**

1. [`docs/HANDOFF.md`](docs/HANDOFF.md) — состояние, грабли, дорожная карта.
2. [`docs/ТЗ.md`](docs/ТЗ.md) — техническое задание.
3. [`docs/D0-measurements.md`](docs/D0-measurements.md) — замеры и принятые решения.

**Коротко:**

- Стек: Rust 1.99 + Slint 1.18, Windows-only, программный рендер по умолчанию.
- Запуск: `cargo run -p posdesktop` (окно), `--overlay` (оверлей).
- Логи: `logs/app.log` в корне проекта.
- Перед коммитом: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`.
- Бюджеты: RSS ≤60 МБ, CPU в покое ≤0.2 %, exe ≤25 МБ.
- Интерфейс и документация — на русском.
- Не начинать крупных изменений без команды владельца.
