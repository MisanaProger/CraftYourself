//! Engineering Skill Matrix catalog (architecture v0.3, §11).
//!
//! `Engineering Profile → Discipline → Skill → Assessment Evidence`.
//! The catalog is data: new disciplines are added here without touching the
//! core. Absence of evidence is reported as `not_assessed`, never as zero.

use serde::Serialize;

/// Version of the catalog and its criteria; stored with every assessment.
pub const CATALOG_VERSION: &str = "esm-catalog-0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisciplineGroup {
    /// A. Fundamental disciplines.
    Fundamental,
    /// B. Engineering disciplines.
    Engineering,
}

/// Dimensions a skill can be assessed on (§11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Theory,
    Practice,
    Diagnosis,
    Transfer,
    Independence,
}

/// Evidence status of a skill or discipline (§11.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentStatus {
    NotAssessed,
    InsufficientEvidence,
    Provisional,
    Established,
    NeedsReassessment,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillDef {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Discipline {
    pub id: &'static str,
    pub name: &'static str,
    pub group: DisciplineGroup,
    pub skills: Vec<SkillDef>,
}

macro_rules! skills {
    ($($id:literal => $name:literal),* $(,)?) => { vec![$(SkillDef { id: $id, name: $name }),*] };
}

pub fn catalog() -> Vec<Discipline> {
    use DisciplineGroup::*;
    vec![
        Discipline {
            id: "algorithms",
            name: "Алгоритмическое мышление",
            group: Fundamental,
            skills: skills![
                "decomposition" => "Декомпозиция задачи",
                "algorithm-design" => "Построение алгоритма",
                "complexity" => "Временная и пространственная сложность",
                "correctness-proof" => "Доказательство корректности",
                "edge-cases" => "Граничные случаи",
                "optimization" => "Оптимизация",
                "strategy" => "Выбор стратегии решения",
            ],
        },
        Discipline {
            id: "data-structures",
            name: "Структуры данных",
            group: Fundamental,
            skills: skills![
                "arrays-lists" => "Массивы и списки",
                "stack-queue" => "Стек и очередь",
                "hash-tables" => "Хеш-таблицы",
                "trees-heaps" => "Деревья и кучи",
                "graphs" => "Графы",
                "fenwick-tree" => "Деревья Фенвика",
                "segment-tree" => "Деревья отрезков",
                "specialized" => "Специализированные структуры",
            ],
        },
        Discipline {
            id: "mathematics",
            name: "Математика",
            group: Fundamental,
            skills: skills![
                "discrete-logic" => "Дискретная математика и логика",
                "linear-algebra" => "Линейная алгебра",
                "calculus" => "Математический анализ",
                "probability-statistics" => "Теория вероятностей и статистика",
                "combinatorics" => "Комбинаторика",
                "geometry" => "Геометрия",
                "number-representation" => "Числовые представления и вычислительная арифметика",
            ],
        },
        Discipline {
            id: "numerical-methods",
            name: "Численные методы",
            group: Fundamental,
            skills: skills![
                "nonlinear-equations" => "Решение нелинейных уравнений",
                "root-localization" => "Локализация и подсчёт корней",
                "interpolation" => "Интерполяция и аппроксимация",
                "differentiation" => "Численное дифференцирование",
                "integration" => "Численное интегрирование",
                "linear-systems" => "Системы линейных уравнений",
                "optimization" => "Численная оптимизация",
                "error-estimation" => "Оценка погрешностей",
                "stability" => "Устойчивость и сходимость",
                "autodiff" => "Автоматическое дифференцирование",
            ],
        },
        Discipline {
            id: "systems-programming",
            name: "Системное программирование",
            group: Engineering,
            skills: skills![
                "memory" => "Память и управление ресурсами",
                "pointers" => "Указатели и ссылки",
                "processes-threads" => "Процессы и потоки",
                "synchronization" => "Синхронизация",
                "filesystems" => "Файловые системы",
                "system-apis" => "Системные API",
                "abi-ffi" => "ABI и FFI",
                "low-level-diagnostics" => "Низкоуровневая диагностика",
            ],
        },
        Discipline {
            id: "computer-graphics",
            name: "Компьютерная графика",
            group: Engineering,
            skills: skills![
                "coordinate-systems" => "Системы координат",
                "vector-matrix" => "Векторная и матричная геометрия",
                "affine-transforms" => "Аффинные преобразования",
                "projections" => "Проекции",
                "rasterization" => "Растеризация",
                "curves-surfaces" => "Кривые и поверхности",
                "shaders" => "Шейдеры",
                "gpu-pipeline" => "GPU pipeline",
                "rendering" => "Рендеринг",
                "spatial-structures" => "Пространственные структуры данных",
            ],
        },
        Discipline {
            id: "web-ui",
            name: "Вёрстка и интерфейсы",
            group: Engineering,
            skills: skills![
                "html" => "HTML",
                "css" => "CSS",
                "adaptive-layout" => "Адаптивная вёрстка",
                "layout-systems" => "Layout systems",
                "typography" => "Типографика",
                "components" => "Композиция компонентов",
                "accessibility" => "Доступность",
                "ui-ux" => "Основы UI/UX",
                "responsive" => "Responsive design",
                "frontend-api" => "Взаимодействие frontend с API",
            ],
        },
        Discipline {
            id: "architecture",
            name: "Архитектура ПО",
            group: Engineering,
            skills: skills![
                "decomposition" => "Декомпозиция систем",
                "modularity" => "Модульность",
                "api-design" => "Проектирование API",
                "dependencies" => "Управление зависимостями",
                "patterns" => "Паттерны",
                "module-boundaries" => "Границы модулей",
                "scalability" => "Масштабируемость",
                "maintainability" => "Сопровождаемость",
                "tradeoffs" => "Архитектурные компромиссы",
            ],
        },
        Discipline {
            id: "debugging-testing",
            name: "Отладка и тестирование",
            group: Engineering,
            skills: skills![
                "reproduction" => "Воспроизведение ошибок",
                "hypotheses" => "Построение гипотез",
                "debugger" => "Debugger",
                "unit-integration" => "Unit и integration tests",
                "property-based" => "Property-based testing",
                "edge-cases" => "Граничные случаи",
                "profiling" => "Профилирование",
                "regression" => "Регрессионное тестирование",
            ],
        },
        Discipline {
            id: "git",
            name: "Инженерная работа с Git",
            group: Engineering,
            skills: skills![
                "commits" => "Коммиты",
                "branching" => "Ветвление",
                "merge-rebase" => "Merge и rebase",
                "conflicts" => "Разрешение конфликтов",
                "diff-analysis" => "Анализ diff",
                "code-review" => "Code review",
                "pull-requests" => "Pull Request",
                "issues" => "Issues",
                "history" => "История изменений",
            ],
        },
    ]
}

pub fn is_known_discipline(id: &str) -> bool {
    catalog().iter().any(|d| d.id == id)
}

/// One row of the matrix as shown to the learner.
#[derive(Debug, Clone, Serialize)]
pub struct MatrixSkill {
    pub id: &'static str,
    pub name: &'static str,
    pub status: AssessmentStatus,
    /// `None` unless there is evidence: no data is not a zero.
    pub score: Option<u8>,
    pub evidence_count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatrixDiscipline {
    pub id: &'static str,
    pub name: &'static str,
    pub group: DisciplineGroup,
    pub status: AssessmentStatus,
    pub score: Option<u8>,
    pub skills: Vec<MatrixSkill>,
    /// Completed tasks that exercised this discipline (not yet evidence).
    pub practice_tasks: u32,
}

/// Builds the matrix. Skill assessment from evidence is not implemented yet,
/// so every skill is `not_assessed`; practice is reported separately.
pub fn matrix(practice: impl Fn(&str) -> u32) -> Vec<MatrixDiscipline> {
    catalog()
        .into_iter()
        .map(|d| MatrixDiscipline {
            id: d.id,
            name: d.name,
            group: d.group,
            status: AssessmentStatus::NotAssessed,
            score: None,
            practice_tasks: practice(d.id),
            skills: d
                .skills
                .into_iter()
                .map(|s| MatrixSkill {
                    id: s.id,
                    name: s.name,
                    status: AssessmentStatus::NotAssessed,
                    score: None,
                    evidence_count: 0,
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique() {
        let c = catalog();
        assert_eq!(c.len(), 10);
        let mut ids: Vec<_> = c.iter().map(|d| d.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10);
        for d in &c {
            let mut s: Vec<_> = d.skills.iter().map(|s| s.id).collect();
            s.sort();
            s.dedup();
            assert_eq!(s.len(), d.skills.len(), "{}", d.id);
        }
    }

    #[test]
    fn missing_data_is_not_zero() {
        let m = matrix(|_| 0);
        assert!(
            m.iter()
                .all(|d| d.score.is_none() && d.status == AssessmentStatus::NotAssessed)
        );
        assert!(m.iter().flat_map(|d| &d.skills).all(|s| s.score.is_none()));
        assert!(is_known_discipline("debugging-testing") && !is_known_discipline("magic"));
    }
}
