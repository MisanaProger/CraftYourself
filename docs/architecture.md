# Архитектура Craft Yourself

Соответствует архитектурному ТЗ **Developer Edition v0.3**. Модульный монолит
на Rust, Local-first, chat-first интерфейс на React + TypeScript.

## Слои и crates

```
 frontend (React)  ──REST /api/v1 · WS /api/v1/events──►  est-api ◄── est-cli (`est`)
                                                            │
                                                     est-application   ← сценарии + порты (traits)
                                                            │
         ┌────────────┬─────────────┬────────────┬──────────┴──┬──────────────┬─────────────┐
     est-storage   est-git    est-projects  est-workspace   est-ai      (адаптеры портов)
     (SQLite)     (git CLI)   + est-languages (~/.est, IDE) (LLM Gateway)
                                                            │
     чистая логика:  est-ai-policy · est-documentation · est-dependency (ADI) ·
                     est-discipline (ESM) · est-exercises (манифест)
                                                            │
                                     est-domain  →  est-core
```

`est-domain` не зависит от SQLx, Axum, Git, контейнеров или LLM.
`est-application` объявляет порты (`ProjectRepository`, `ConversationRepository`,
`EngagementRepository`, `AdiRepository`, `GitPort`, `LlmProvider`, …);
инфраструктурные crates их реализуют; `est-api::runtime` — composition root.

| Crate | Модуль ТЗ v0.3 | Содержимое |
|---|---|---|
| `est-core` | §4.1 | ID, `DomainEvent`, имена событий |
| `est-domain` | §5, §7 | Project, Task, LearningSession, Conversation, Message, EngagementLevel/State, Escalation |
| `est-application` | §3 | сценарии: проекты, Issues, сессии, Conversation Engine, ADI, ESM, экспорт |
| `est-ai-policy` | §7–8 | классификация запроса, требуемый уровень, одношаговая эскалация, ограничения ответа, разрешения инструментов |
| `est-documentation` | §8.3 | Documentation Router |
| `est-ai` | §8.1 | Провайдеры LLM: OpenAI-совместимый (Ollama, Gemini, DeepSeek, Kimi, Qwen…), OpenAI, Claude Messages API; каталог пресетов |
| `est-dependency` | §10 | ADI: оценка Issue по событиям, двойное затухание, игровые статусы |
| `est-discipline` | §11 | каталог ESM, статусы доказательности |
| `est-exercises` | §13 | Exercise Manifest |
| `est-languages`, `est-projects` | §15 | языковые адаптеры (только описание команд), лицензия |
| `est-workspace` | §16 | `~/.est`, рабочие каталоги, запуск внешней IDE |
| `est-git` | §14 | безопасная обёртка над git CLI |
| `est-sandbox` | §17 | запуск команд в Docker/Podman: без сети по умолчанию, лимиты, read-only |
| `est-learning` | §18 | программы, уроки, тесты, артефакты, шаблоны практики из `learning-artifacts/` |
| `est-storage` | §19 | SQLite, миграции `migrations/` |
| `est-api`, `est-cli` | §22, §16 | REST, WebSocket, Host/Origin-защита; CLI `est` |
| `apps/desktop` | §16, §21 | десктопное приложение на Tauri 2: демон в том же процессе на loopback-порту, встроенный интерфейс, без IPC |

## Сквозной сценарий (§30)

1. `POST /tasks/{id}/start` — рабочий каталог `~/.est/workspaces/<project>/<task>`,
   ветка `est/<task>`, диалог Issue и **engagement L0**.
2. `POST /conversations/{id}/messages` — Policy Engine до любой модели:
   * вопрос об API → карточка `sources` (исходники, документация), модель не вызывается;
   * запрос выше уровня → `engagement.escalation.proposed` на **одну** ступень, запрос заблокирован;
   * иначе → AI Gateway с системным промптом уровня; превышение лимита кода вырезается.
3. `POST /engagement/escalations/{id}/accept|reject` — согласие поднимает уровень на
   ступень и повторно обрабатывает исходный запрос; отказ — ответ в пределах уровня.
   `POST /conversations/{id}/engagement/raise` — пользователь сам просит следующую ступень.
4. Работа во внешней IDE; EST видит её через Git (`/sessions/{id}`, `/diff?against=base`).
5. `POST /tasks/{id}/complete` — финальный ADI (`scoring_version = observed-0.1`).
6. `/analytics/adi`, `/analytics/disciplines` — независимые ADI и ESM.

## Решения, принятые при реализации

- **Концептуальные вопросы** объясняются на любом уровне (§8.3); уровнем
  ограничена только помощь с решением задачи.
- **Определение требуемого уровня** — детерминированные правила
  (`est-ai-policy/src/engagement.rs`), а не модель: модель не может обойти политику.
- **ADI v0** (`observed-0.1`) считается только по наблюдаемым событиям:
  ответы ИИ по уровням, собственные коммиты, отклонённые эскалации,
  концептуальные вопросы (малый вес). Сама эскалация без использования помощи
  ADI не повышает. Структурированная оценка модели — следующий шаг.
- **Повторное завершение Issue** заменяет её ADI-оценку и ставит её в конец
  порядка (для затухания по числу Issue); равные timestamp упорядочиваются по номеру.
- **Удаление диалога** удаляет сообщения, эскалации, состояние engagement,
  события диалога и ADI задачи; диалог активной Issue удалить нельзя.
- **Локальный daemon** принимает запросы только с собственным `Host` и
  одноимённым `Origin` (защита от DNS rebinding и CSRF).
- **Облачные провайдеры (AI Gateway).** Ученик подключает Claude, ChatGPT,
  Gemini, DeepSeek, Kimi, Qwen, кодинг-планы (Kimi Code, Qwen Coding Plan),
  Ollama или любой OpenAI-совместимый API в «Настройки → Нейросети».
  Список провайдеров — `~/.est/config/providers.toml` (без секретов); выбор
  там важнее `[llm]` из `config.toml`. Активный провайдер лежит в
  `LlmSlot` и переключается без перезапуска; смена пишет событие
  `ai.provider.changed` (имя, модель, endpoint, remote — без ключа).
  Политика не зависит от провайдера: уровень, лимиты и граница документации
  решаются до вызова модели.
- **Ключи, введённые в UI,** хранятся в `~/.est/secrets/llm/<id>.key`
  (каталог 0700, файл 0600) — отдельно от конфигурации, событий, логов и
  экспорта; API только записывает и удаляет ключ и сообщает, есть ли он.
  Переменная окружения из настроек провайдера по-прежнему поддерживается.
  Потребительские подписки (Claude Pro/Max, ChatGPT Plus/Pro, Google AI) не
  дают API-доступа сторонним приложениям и не поддерживаются; «подписка»
  здесь — кодинг-планы, выдающие собственный ключ.

- **Программы по запросу** (`est-application/src/generation.rs`,
  `est-learning/src/generated.rs`). `POST /learning/generated` с темой,
  уровнем и числом уроков (3–10) запускает генерацию в фоне (202): модель
  пишет план, затем каждый урок. Что примет система, решает код: ответ
  разбирается и проверяется (JSON плана и теста, Markdown урока, HTML-документ
  страницы), id уроков и вопросов назначает код, размеры ограничены, вопрос с
  неверным ключом отбрасывается; негодный ответ повторяется один раз с причиной
  отказа, затем генерация завершается ошибкой. Хранится в `generated_programs`
  (миграция 0004) с id `gen-<uuid>` и дальше обслуживается теми же
  `/learning/programs/{id}/…`, что и встроенные; ответы тестов не покидают
  сервер, в экспорт попадают только тема, название и список уроков. Генерация —
  учебный материал, а не помощь по Issue: она не связана с диалогом, не
  меняет уровень engagement и не влияет на ADI. Прерванная перезапуском
  генерация помечается как failed. События `learning.program.generation.*`.
- **Формулы LaTeX** (`$…$`, `$$…$$`, `\(…\)`, `\[…\]`) в Markdown рендерит
  KaTeX: он строит DOM-узлы сам (`katex.render`, без `innerHTML`), с
  `trust: false` и лимитами размера; библиотека грузится при первом
  использовании.
- **HTML5-страницы от модели** — новый вид урока `page` в программах по
  запросу и блоки ```` ```html ```` в ответах ментора. Страница никогда не
  вставляется в DOM приложения: сервер отдаёт её как изолированный документ
  с тем же CSP `sandbox allow-scripts`, что и артефакты
  (`/learning/programs/{id}/lessons/{lesson}/page`,
  `/conversations/{id}/messages/{message}/pages/{n}` — n-й html-блок
  сохранённого ответа ассистента), и добавляет скрипт `est:resize`; интерфейс
  показывает её в `<iframe sandbox="allow-scripts">` и принимает только
  `est:resize`/`est:progress` версии 1. Политика (`est-ai-policy`,
  `ResponseConstraints::max_pages`): страницы в ответах ментора — только с
  уровня L2, для любого вида вопроса: одна страница до 400 строк / 64 КБ вне
  бюджета кода. На L0–L1 страницы нет даже для концептуального вопроса и даже
  короткой: её блок вырезается (`policy.page_removed`), остальной ответ
  сохраняется. Сервер отдаёт только страницы, прошедшие политику
  (`meta.pages` сообщения). Переводы и критерии приёмки страниц не содержат.
  Страницы уроков сгенерированных программ к уровню не привязаны: это учебный
  материал, как встроенные артефакты.
- **Совместимость страниц модели** (`est-api/src/routes/isolated.rs`). Модель
  пишет страницу как для обычной вкладки, а в песочнице часть API недоступна.
  Перед скриптами страницы сервер вставляет слой: `localStorage`/`sessionStorage`
  в памяти (в opaque origin они бросают исключение); `alert`/`confirm`/`prompt`
  — сообщение внутри страницы (без `allow-modals`, который позволил бы
  заблокировать вкладку); отправка формы вызывает обработчики `submit`
  (песочница без `allow-forms` блокирует её до события, `form-action 'none'`
  не даёт ничего отправить); тема приложения из `?theme=` — в `data-theme`
  и `color-scheme`; подгонка высоты, которая останавливается у страниц,
  растянутых на `100vh`. Для страниц модели CSP разрешает `eval` — код в них и
  так произвольный, изоляцию дают песочница и запрет сети. Web Workers и сеть
  недоступны. Проверено в Chromium: анимации (rAF, CSS, SMIL), canvas,
  таймеры, мышь и клавиатура, Web Audio, формы, сообщения, хранилище, тема.

## Состояние этапов (§24)

| Этап | Статус |
|---|---|
| 0. Foundation | ✅ workspace, React, REST, SQLite + миграции, конфиг, логирование, CI |
| 1. Project & Workspace | ✅ локальные репозитории, языки, рабочие каталоги, IDE, status/diff |
| 2. Language & Sandbox | ✅ адаптеры Rust/C/C++/Python/JS, песочница Docker/Podman с лимитами и изоляцией, разбор итогов тестов |
| 3. Task & Issue Engine | ✅ задачи, импорт упражнений, статусы, сессии; ◯ импорт GitHub Issues |
| 4. Chat-first Conversation Engine | ✅ диалоги, история, события, WebSocket; ◯ потоковая генерация |
| 5. AI Gateway & Engagement | ✅ провайдер, L0–L4, эскалация с согласием, Documentation Boundary; ◯ permission gates внешних действий (таких действий пока нет) |
| 6. Conversation Intelligence | ◯ Session Summary, Knowledge Profile, рекомендации; ✅ экспорт/удаление |
| 7. ADI & ESM | ✅ ADI с двойным затуханием, каталог ESM; ◯ оценка навыков по evidence |
| 8. Interactive Learning | ◐ программы, уроки, тесты с проверкой на сервере, артефакты в sandboxed iframe, практические проекты, программы по запросу (генерация нейросетью), LaTeX и изолированные HTML-страницы в уроках и чате; ◯ остальные программы |
| 9. Git Workflow & Analytics | ◯ |
