# Спецификация: граф инженерных компетенций

- **Статус:** Draft
- **ТЗ:** 2.2 · **Roadmap:** TEC-021 (TEC-022 … TEC-025), LRN-015

## Цель

Описать зависимости между навыками, чтобы видеть незакрытые предпосылки и
критические пробелы и учитывать их при подборе Issue и пути обучения.

## Модель

```
Discipline ─┬─ Competency ─┬─ Skill ─┬─ KnowledgeUnit
            │              │         └─ prerequisite → Skill | KnowledgeUnit
            │              └─ prerequisite → Competency
            └─ Assessment, Issue (ссылки по SkillRef)
```

- `Discipline` и `Skill` — существующие узлы `esm-catalog-0.2`; их id
  (`algorithms/complexity`) не меняются.
- `Competency` — группа навыков внутри дисциплины (например,
  `numerical-methods/root-finding`).
- `KnowledgeUnit` — проверяемая единица знания (например, «условие
  сходимости метода Ньютона»); на неё ссылаются вопросы тестов и критерии.
- `Prerequisite { from, to, strength: required | helpful }`.
- Assessment и Issue не хранятся в графе: они ссылаются на узлы через
  `SkillRef`, оценка узла берётся из журнала свидетельств.

## Каталог как данные (TEC-022)

Каталог переносится из литералов Rust в TOML (`esm-catalog-0.3`), встроенный
в crate (`include_str!`), с именами `ru/en/uz`. Версия каталога пишется в
каждую запись свидетельства и оценку.

```toml
[[competency]]
id = "numerical-methods/root-finding"
skills = ["numerical-methods/nonlinear-equations", "numerical-methods/root-localization"]
prerequisites = [
  { to = "mathematics/calculus", strength = "required" },
  { to = "numerical-methods/error-estimation", strength = "required" },
  { to = "algorithms/algorithm-design", strength = "helpful" },
]
```

## Проверки (TEC-023)

Тест `est-discipline`: нет циклов по `required`-предпосылкам; все ссылки
существуют; у каждого узла имена на трёх языках; каждый навык 0.2
разрешается в 0.3.

## Запросы (TEC-024)

Чистые функции (граф + оценки навыков):

- `unmet_prerequisites(skill)` → список `(узел, причина)`; причина —
  `not_assessed`, `insufficient_evidence`, `below_threshold(score)`,
  `needs_reassessment`;
- `critical_gaps(target)` — незакрытые `required`-предпосылки на всех
  путях к цели, по числу целей, которые они блокируют (основа
  bottleneck detection, LRN-026);
- `exercised(issue)` — навыки Issue (названные или выведенные, как
  `infer_skills`).

## Пример ТЗ 2.2 — численные методы (LRN-017)

`numerical-methods/nonlinear-equations` требует: `mathematics/calculus`
(производные), `numerical-methods/error-estimation` (погрешности
вычислений), `algorithms/algorithm-design` (алгоритмизация),
`numerical-methods/stability` (численная устойчивость); производные как
единица знания — `KnowledgeUnit` внутри `mathematics/calculus`. Недостаток, например, в погрешностях даёт
`unmet_prerequisites` с причиной, и подбор Issue (LRN-025) сначала
предлагает задачу на погрешности. Этот пример — фикстура теста TEC-024.

## Ограничения

Проверка предпосылок информирует, но не запрещает работу (TEC-029): ученик
может начать Issue с незакрытыми предпосылками — это наблюдаемый факт для
адаптации, а не блокировка.
