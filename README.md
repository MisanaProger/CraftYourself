<p align="center"><img src="docs/design/logo.jpg" alt="Craft Yourself — Build skills. Not dependencies." width="520"></p>

# Craft Yourself

Open-source тренажёр инженерных навыков: реальные проекты и Issues, внешняя
IDE, чат с ИИ-наставником, который **помогает рассуждать, а не решает за вас**.

> **Build skills. Not dependencies.**

Архитектура: [`docs/architecture.md`](docs/architecture.md) (по ТЗ v0.3).
Концепты интерфейса: [`docs/design/`](docs/design).

## Что уже работает

- **Проекты и рабочие каталоги.** Подключение локального Git-репозитория (или
  клонирование удалённого), определение языков и лицензии, отдельный рабочий
  каталог и ветка на каждую Issue в `~/.est/workspaces`, открытие во внешней IDE.
- **Issues.** Ручное создание и импорт упражнений (Exercise Manifest, TOML),
  статусы, привязка к дисциплинам Skill Matrix, завершение Issue.
- **Chat-first ментор.** Диалог на каждую Issue; сообщения и события хранятся
  в SQLite и переживают перезапуск.
- **Engagement Controller L0–L4.** Новая Issue всегда начинается с L0. Запрос
  выше текущего уровня блокируется, и Craft Yourself предлагает подняться на
  **одну** ступень; ответ повышенного уровня возможен только после явного
  согласия. Ограничения проверяются кодом: модель не может сама поднять
  уровень, а лишний код в ответе вырезается политикой.
- **Documentation Boundary.** Вопросы о конкретных API не уходят в модель:
  ответ — ссылки на определение в исходниках, документацию задачи и API
  Reference. Скрытые тесты упражнения наставнику недоступны.
- **AI Gateway.** Любой OpenAI-совместимый endpoint: локальная Ollama,
  llama.cpp server или внешний API (только https, ключ — из переменной
  окружения). Без модели политика работает, но ответов модели нет.
- **ADI.** Финальный индекс зависимости от ИИ после завершения Issue — по
  наблюдаемым событиям, с факторами и уверенностью; среднее с двойным
  затуханием (30 дней / 10 Issue), игровой статус с обязательной оговоркой.
- **Engineering Skill Matrix.** Каталог из 10 дисциплин; непроверенный навык
  показывается как «не оценено», а не 0. ADI и ESM независимы.
- **Приватность.** Данные локально; экспорт всей истории в JSON; удаление
  диалога вместе с событиями и ADI задачи.

Пока **нет**: Sandbox Manager (запуск тестов в контейнере), Session Summary и
Knowledge Profile, оценка навыков по evidence, интерактивные материалы, проверка
коммитов и PR. См. «Состояние этапов» в архитектуре.

## Быстрый старт

Нужны Rust ≥ 1.85, `git` и Node.js 22 (для интерфейса).

```sh
cargo build --release
(cd frontend && npm ci && npm run build)

./target/release/est init                 # ~/.est и config.toml
./target/release/est project add ~/src/my-project
./target/release/est serve                # http://127.0.0.1:7878
```

Модель подключается в `~/.est/config/config.toml`:

```toml
[llm]
base_url = "http://127.0.0.1:11434/v1"   # Ollama
model = "qwen2.5-coder:7b"
```

### CLI

```sh
est task import exercises/examples/globset-escape-roundtrip.toml
est task start <id> --open        # рабочий каталог + IDE
cd "$(est workspace path)"
est chat "Нашёл функцию escape, что проверить дальше?"
est escalation accept             # или reject
est workspace status
est task complete <id>            # финальный ADI
est adi
est data export --output history.json
```

## Разработка

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cd frontend && npm run dev        # http://localhost:5173 → API на :7878
```

## Лицензия

Файл [LICENSE](LICENSE) — GPL-3.0. В ТЗ v0.3 лицензия проекта отмечена как
TBD; окончательный выбор за владельцем репозитория.
