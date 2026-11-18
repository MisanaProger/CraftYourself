# Архитектура

Модульный монолит на Rust (ТЗ §8.1). Каждая подсистема — отдельный crate с
явным публичным API; зависимости направлены только «вниз».

```
                      est-server  (Axum, composition root, REST /api)
          ┌──────────────┬─────────────┼──────────────┬──────────────┐
     est-storage     est-git      est-exercise     est-policy
   (SQLx/SQLite)   (git CLI)    (Exercise Manifest) (AI Policy Engine)
          └──────────────┴─────────────┴──────────────┘
                             est-core  (общие доменные типы)
```

| Crate          | Подсистема ТЗ                                     | Назначение |
|----------------|---------------------------------------------------|------------|
| `est-core`     | —                                                 | `HelpLevel`, `Difficulty`, `TaskCategory`, `Competency` |
| `est-exercise` | Exercise Engine (§3.3)                            | Разбор и валидация манифеста, версия формата |
| `est-git`      | Git Engine (§3.5)                                 | Обёртка над git CLI для недоверенных репозиториев |
| `est-policy`   | AI Policy Engine, Documentation Router (§4.2–4.4) | Разрешения инструментов, уровни помощи, граница документации |
| `est-storage`  | Хранилище (§8.2)                                  | SQLite, миграции, журнал событий |
| `est-server`   | Core Application, Project Manager, Workspace Manager | HTTP API |

## Ключевые решения

**Политики наставника — код, а не промпт (§4.3, §4.4).** `POST
/api/sessions/{id}/mentor/route` классифицирует вопрос, применяет
Documentation Boundary и назначает уровень помощи *до* обращения к модели.
Вопросы-справки об API к модели не попадают вообще (`forward_to_model:
false`), вместо ответа возвращаются ссылки на первоисточники. Классификатор
основан на правилах: он детерминирован и проверяем, и его нельзя уговорить.

**Недоверенные репозитории (§11).** Git запускается без shell, с
`core.hooksPath=/dev/null`, `protocol.ext.allow=never`, без глобальной
конфигурации и интерактивных запросов; аргументы, похожие на опции, отклоняются.
Схемы URL ограничены `https`/`ssh` (локальные пути — только по явному флагу).
Файловое API рабочей копии отклоняет абсолютные пути, `..`, `.git` и
симлинки, ведущие наружу. **Код проектов (сборка, тесты) пока не
запускается**: это произойдёт только внутри Sandbox Manager (Podman/Docker).

**Воспроизводимость упражнений (§3.3).** Манифест закрепляет полный commit
hash; сессия создаёт ветку `est/<exercise-id>` от этой ревизии, поэтому
`git diff base..HEAD` — это ровно работа обучаемого.

**Оценка по наблюдаемым действиям (§7).** Таблица `events` — неизменяемый
журнал: `session.started`, `git.commit`, `mentor.request` (тип вопроса,
запрошенный и выданный уровень). Из него будут строиться метрики AI Dependency,
Coding Independence и др.

**Local Edition без регистрации (§9).** При первом запуске создаётся
неявный локальный пользователь; модель данных уже содержит `user_id` для Cloud Edition.

## REST API (v0)

| Метод | Путь | Описание |
|-------|------|----------|
| GET  | `/api/health`, `/api/me` | Состояние, текущий пользователь |
| GET/POST | `/api/projects` | Список / подключить репозиторий `{url, branch?, description?}` |
| GET  | `/api/projects/{id}`, `/api/projects/{id}/files` | Проект, структура файлов |
| GET/POST | `/api/exercises` | Список / регистрация (тело — TOML-манифест) |
| GET  | `/api/exercises/{id}` | Упражнение с разобранным манифестом |
| GET/POST | `/api/sessions` | Список / начать `{exercise_id}` |
| GET  | `/api/sessions/{id}`, `/events` | Сессия, журнал событий |
| POST | `/api/sessions/{id}/status` | `{status: active\|completed\|abandoned}` |
| GET  | `/api/sessions/{id}/files` | Отслеживаемые файлы |
| GET/PUT | `/api/sessions/{id}/files/{path}` | Чтение / запись файла |
| GET  | `/api/sessions/{id}/git/{status,diff,log,branches}` | `diff?against=worktree\|staged\|base`, `log?limit=` |
| POST | `/api/sessions/{id}/git/add`, `/git/commit` | `{paths}`, `{message}` |
| POST | `/api/sessions/{id}/mentor/route` | `{question, excerpt?, level?}` → решение политики и ссылки |

## Состояние этапов

| Этап | Сделано | Осталось |
|------|---------|----------|
| 1. Foundation | Архитектура, Git, проекты, манифест, сессии, SQLite, локальный пользователь | Frontend (SvelteKit), PostgreSQL, поиск проектов на GitHub |
| 2. AI Mentor | Policy Engine, Documentation Boundary, уровни помощи, журнал запросов | AI Gateway (llama.cpp / Ollama / внешние API), история диалога |
| 3. Development Workflow | Diff, коммиты, файловое API | Sandbox Manager и запуск тестов, терминал, Commit/PR/Issue Assistant |
| 4–6 | — | Interactive Learning, Knowledge & Analytics, Cloud Edition |
