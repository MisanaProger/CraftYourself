# Спецификация: расширенный жизненный цикл Issue

- **Статус:** Draft
- **ТЗ:** 2.8, 2.9 · **Roadmap:** TEC-026 (TEC-027 … TEC-033), TEC-046

## Этапы

```
Creation → Validation → Prerequisite Check → Investigation → Solution →
Testing → Reflection → Assessment → Skill Evidence → Progress Update
```

Этапы — слой поверх существующих `TaskStatus` (open, in-progress, done,
archived) и сессий: статус остаётся грубым состоянием, этап — где ученик
сейчас. Соответствие:

| TaskStatus | Этапы |
|---|---|
| open | Creation, Validation, Prerequisite Check |
| in-progress | Investigation, Solution, Testing, Reflection |
| done | Assessment, Skill Evidence, Progress Update (выполняются кодом при `POST /tasks/{id}/complete`) |

## Переходы

- **Validation (TEC-028)** — фиксированные правила: есть описание, критерии
  приёмки проверяемы (не пустые, не дублируются), команды тестов
  объявлены для практических задач, навыки из каталога. Для импортированных
  Issue — плюс правила пригодности `est-issues`. Ошибка блокирует старт,
  предупреждение — нет.
- **Prerequisite Check (TEC-029)** — `unmet_prerequisites` для навыков Issue;
  показывается с предложенными уроками, работу не блокирует.
- **Investigation → Solution → Testing** — определяются наблюдаемыми
  событиями: первый собственный коммит переводит в Solution, первый прогон
  тестов — в Testing; ученик может отметить этап сам. Используются для
  бюджета подсказок по этапам (TEC-050).
- **Reflection** — обязателен для типов Issue, выбранных в манифесте или по
  правилу (algorithm, bug-fix); ответы сохраняет TEC-046. Пропуск
  разрешён, но фиксируется.
- **Assessment → Skill Evidence → Progress Update** — существующий ADI,
  затем запись свидетельств (TEC-013/014), затем обновление retention и
  траектории.

Каждый переход — событие `task.stage.changed` (`est_core::events`) с id
задачи и сессии. Engagement не зависит от этапа: новая Issue — L0.

## Зависимости и ветвление (TEC-030)

- `depends_on` между задачами; незавершённая зависимость — предупреждение.
- Сценарии: манифест может объявить несколько путей (`[[scenario]]` с
  критериями), выбранный путь записывается в сессию.

## Replay (TEC-031)

Повторное прохождение создаёт новую сессию и новую ADI-оценку; прежние
сохраняются (сейчас заменяются). Агрегат ADI использует последнюю оценку
Issue, история — все. Свидетельства повторного прохождения получают тип
`repeated_application`, если прошёл хотя бы день.

## Генерация по пробелам (TEC-032)

Модель предлагает черновик задачи для пробела (навык, предпосылка,
заблуждение); черновик проходит Validation как любая Issue и начинается с L0.

## Схема (TEC-033)

`docs/schemas/issue-schema.json` — JSON Schema манифеста и записей этапов;
тест проверяет примеры из `exercises/examples/`.
