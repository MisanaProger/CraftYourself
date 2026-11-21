# Спецификация: доказательная оценка, provenance, версии правил

- **Статус:** Draft
- **ТЗ:** 2.1, 3.6, 4.4, 4.5 · **Roadmap:** LRN-011, TEC-011, TEC-017, PRD-032, PRD-046
- **Решение:** [ADR-0002](../decisions/ADR-0002-evidence-ledger.md)

## Цель

Оценка навыка — функция от сохранённых свидетельств и версии правил. Любую
оценку можно объяснить (какие записи, как посчитано) и воспроизвести (та же
версия → тот же результат). Это развивает существующую Skill Matrix
(`esm-catalog-0.2`), а не заменяет её: пять измерений, статусы и
`not_assessed` сохраняются.

## <a id="record"></a>Запись свидетельства

```rust
pub struct EvidenceRecord {
    pub id: EvidenceId,                 // uuid
    pub user_id: UserId,
    pub subject: Subject,               // Issue (task + session) | Lesson (program/lesson) | ControlTask
    pub skill: SkillRef,                // discipline[/skill], разрешено по каталогу на момент наблюдения
    pub catalog_version: String,        // "esm-catalog-0.2" …
    pub evidence_type: EvidenceType,    // см. ниже
    pub dimension: Dimension,           // Theory | Practice | Diagnosis | Transfer | Independence
    pub score: u8,                      // 0–100
    pub observed_at: Timestamp,
    pub help: HelpUsed,                 // max_level, hints_by_stage, copied_code_share
    pub check: CheckResult,             // method + outcome (tests passed 7/8, quiz 80 %, …)
    pub reliability: Reliability,       // High | Medium | Low
    pub context: SourceContext,         // lesson | problem | project:<id>; event ids
    pub rule_version: String,           // версия правил выведения, напр. "evidence-1"
    pub supersedes: Option<EvidenceId>, // исправление более ранней записи
}
```

Поля ТЗ 2.1 → поля записи: идентификатор пользователя → `user_id`; Issue →
`subject`; навык → `skill`; тип → `evidence_type`; дата → `observed_at`;
уровень помощи → `help`; результат проверки → `check`; достоверность →
`reliability`; ссылка на исходный контекст → `context` (включая id событий
`session_events`).

Инварианты (`est-domain`, модульные тесты): `score ≤ 100`; `skill`
существует в `catalog_version`; `observed_at` не в будущем; `supersedes`
указывает на запись того же пользователя и навыка.

## <a id="types"></a>Типы свидетельств (LRN-012)

| Тип | Наблюдаемое условие | Измерение |
|---|---|---|
| `independent_solution` | Issue/практика завершена, тесты прошли, уровень помощи ≤ L1, скопированного кода нет | Practice, Independence |
| `hinted_solution` | то же, но использованы ответы L2+ или скопирован код | Practice, Independence (балл по уровню, как сейчас) |
| `self_correction` | упавший тест или замечание ревью позже исправлены самим учеником без ответа L3+ между ними | Diagnosis |
| `explanation` | проверенный ответ: тест на понимание, оценённый сервером, или рефлексия, подтверждённая проверкой | Theory |
| `repeated_application` | тот же навык снова применён успешно, в том же контексте, не раньше чем через 1 день | добавляется к базовому типу |
| `new_context_application` | применение в другом контексте (правило Transfer; v2 — LRN-034) | Transfer |
| `control_task` | контрольная задача (retention, проверка), выполненная на L0 | измерение задачи |

Типы взаимоисключающие для пары (subject, skill), кроме `repeated_application`
и `new_context_application`, которые добавляются к базовой записи. Чтение,
прерванная Issue и отсутствие измерения свидетельствами не являются
(как сейчас).

## <a id="reliability"></a>Достоверность (LRN-013)

| Уровень | Метод проверки |
|---|---|
| High | автоматические тесты в песочнице, тест, оценённый сервером, контрольная задача на L0 |
| Medium | критерии приёмки, отмеченные учеником; ревью коммитов фиксированными правилами |
| Low | самоотчёт, рефлексия без проверки, оценка с участием модели |

Low-свидетельства сами по себе не делают навык `established`: правило статуса
требует минимум одно High/Medium среди трёх объектов.

## Выведение (TEC-013)

Чистые функции в `est-discipline`, вход — `SessionSummary`, события сессии,
попытка теста, результат артефакта; выход — `Vec<EvidenceRecord>`. Без I/O,
без модели, без ADI. Одна Issue даёт записи по каждому целевому навыку
(`issue_targets` как сейчас) и измерению. Свойство-тест: перестановка
независимых событий не меняет результат.

## Хранение (TEC-014)

Таблица новой миграции (`evidence_records`), append-only; индекс
`(user_id, skill, observed_at)`. Порт `EvidenceRepository` в
`est-application/src/ports.rs`, адаптер в `est-storage`. Записи создаются в
момент наблюдения (завершение сессии, попытка теста) — навыки урока
фиксируются тогда, а не при запросе матрицы.

## <a id="provenance"></a>Provenance и версии (TEC-018, TEC-019)

Каждая оценка (снимок навыка в Skill Matrix, ADI-оценка) хранит:

```json
{
  "rule": "esm-assess", "rule_version": "esm-catalog-0.3/assess-2",
  "trigger": { "kind": "session.finished", "event_ids": ["…"] },
  "evidence_ids": ["…"],
  "inputs": { "latest_per_reference": [["ref", "practice", 80, "high"]] },
  "model": null
}
```

`model` заполняется только когда модель участвовала (объяснение ADI) —
имя провайдера/модели без секретов. Реестр версий правил живёт в коде;
пересчёт сохранённой оценки её версией возвращает сохранённый результат
(тест). Смена версии не меняет сохранённые оценки молча: пересчёт —
явное действие с записью.

Воспроизводимость задач (TEC-020, ТЗ 3.6): попытка хранит версии условия,
тестов, критериев, digest образа песочницы, параметры генерации и версию
правил оценки.

## API (TEC-016)

- `GET /analytics/skills/{discipline}/{skill}/evidence?lang=` — записи и
  вычисление (rule, version, inputs, score).
- `GET /analytics/disciplines/{discipline}/evidence` — то же для дисциплины.

## Миграция данных (TEC-015)

Однократный backfill из `lesson_results`, `learning_progress` и
`session_summaries` с `rule_version = "backfill-1"` и `reliability`,
отмечающей отсутствующие поля. Критерий: матрица по записям равна
нынешней на фикстурах.

## <a id="privacy"></a>Приватность (PRD-046)

Записи входят в экспорт; удаление диалога удаляет записи его сессий (как
сводки); «удалить всё» удаляет все записи. При выключенной аналитике записи
продолжают создаваться для Skill Matrix (это оценка, которую видит сам
ученик), но не передаются в снимок для рекомендаций и подбора.

## Критерии приёмки

Соответствуют элементам TEC-012 … TEC-019 и PRD-032 в
[Technical Roadmap](../roadmaps/technical.md#tec-011) и
[Product Roadmap](../roadmaps/product.md#prd-032).
