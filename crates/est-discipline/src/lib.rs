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
    pub name: Names,
}

#[derive(Debug, Clone, Serialize)]
pub struct Discipline {
    pub id: &'static str,
    pub name: Names,
    pub group: DisciplineGroup,
    pub skills: Vec<SkillDef>,
}

macro_rules! skills {
    ($($id:literal => $name:expr),* $(,)?) => { vec![$(SkillDef { id: $id, name: $name }),*] };
}

/// Names in `ru`, `en`, `uz` order.
pub type Names = [&'static str; 3];

pub fn name_in(names: &Names, lang: &str) -> &'static str {
    match lang {
        "en" => names[1],
        "uz" => names[2],
        _ => names[0],
    }
}

pub fn catalog() -> Vec<Discipline> {
    use DisciplineGroup::*;
    vec![
        Discipline {
            id: "algorithms",
            name: [
                "Алгоритмическое мышление",
                "Algorithmic thinking",
                "Algoritmik fikrlash",
            ],
            group: Fundamental,
            skills: skills![
                "decomposition" => ["Декомпозиция задачи", "Problem decomposition", "Masalani bo‘laklarga ajratish"],
                "algorithm-design" => ["Построение алгоритма", "Designing an algorithm", "Algoritm tuzish"],
                "complexity" => ["Временная и пространственная сложность", "Time and space complexity", "Vaqt va xotira murakkabligi"],
                "correctness-proof" => ["Доказательство корректности", "Proof of correctness", "To‘g‘rilikni isbotlash"],
                "edge-cases" => ["Граничные случаи", "Edge cases", "Chegaraviy holatlar"],
                "optimization" => ["Оптимизация", "Optimisation", "Optimallashtirish"],
                "strategy" => ["Выбор стратегии решения", "Choosing a solution strategy", "Yechim strategiyasini tanlash"],
            ],
        },
        Discipline {
            id: "data-structures",
            name: [
                "Структуры данных",
                "Data structures",
                "Ma’lumotlar tuzilmalari",
            ],
            group: Fundamental,
            skills: skills![
                "arrays-lists" => ["Массивы и списки", "Arrays and lists", "Massivlar va ro‘yxatlar"],
                "stack-queue" => ["Стек и очередь", "Stacks and queues", "Stek va navbat"],
                "hash-tables" => ["Хеш-таблицы", "Hash tables", "Xesh-jadvallar"],
                "trees-heaps" => ["Деревья и кучи", "Trees and heaps", "Daraxtlar va uyumlar"],
                "graphs" => ["Графы", "Graphs", "Graflar"],
                "fenwick-tree" => ["Деревья Фенвика", "Fenwick trees", "Fenvik daraxtlari"],
                "segment-tree" => ["Деревья отрезков", "Segment trees", "Segmentlar daraxtlari"],
                "specialized" => ["Специализированные структуры", "Specialised structures", "Maxsus tuzilmalar"],
            ],
        },
        Discipline {
            id: "mathematics",
            name: ["Математика", "Mathematics", "Matematika"],
            group: Fundamental,
            skills: skills![
                "discrete-logic" => ["Дискретная математика и логика", "Discrete mathematics and logic", "Diskret matematika va mantiq"],
                "linear-algebra" => ["Линейная алгебра", "Linear algebra", "Chiziqli algebra"],
                "calculus" => ["Математический анализ", "Calculus", "Matematik analiz"],
                "probability-statistics" => ["Теория вероятностей и статистика", "Probability and statistics", "Ehtimollar nazariyasi va statistika"],
                "combinatorics" => ["Комбинаторика", "Combinatorics", "Kombinatorika"],
                "geometry" => ["Геометрия", "Geometry", "Geometriya"],
                "number-representation" => ["Числовые представления и вычислительная арифметика", "Number representation and computer arithmetic", "Sonlarning ifodalanishi va hisoblash arifmetikasi"],
            ],
        },
        Discipline {
            id: "numerical-methods",
            name: ["Численные методы", "Numerical methods", "Sonli usullar"],
            group: Fundamental,
            skills: skills![
                "nonlinear-equations" => ["Решение нелинейных уравнений", "Solving nonlinear equations", "Chiziqsiz tenglamalarni yechish"],
                "root-localization" => ["Локализация и подсчёт корней", "Locating and counting roots", "Ildizlarni aniqlash va sanash"],
                "interpolation" => ["Интерполяция и аппроксимация", "Interpolation and approximation", "Interpolyatsiya va approksimatsiya"],
                "differentiation" => ["Численное дифференцирование", "Numerical differentiation", "Sonli differensiallash"],
                "integration" => ["Численное интегрирование", "Numerical integration", "Sonli integrallash"],
                "linear-systems" => ["Системы линейных уравнений", "Systems of linear equations", "Chiziqli tenglamalar tizimlari"],
                "optimization" => ["Численная оптимизация", "Numerical optimisation", "Sonli optimallashtirish"],
                "error-estimation" => ["Оценка погрешностей", "Error estimation", "Xatoliklarni baholash"],
                "stability" => ["Устойчивость и сходимость", "Stability and convergence", "Barqarorlik va yaqinlashish"],
                "autodiff" => ["Автоматическое дифференцирование", "Automatic differentiation", "Avtomatik differensiallash"],
            ],
        },
        Discipline {
            id: "systems-programming",
            name: [
                "Системное программирование",
                "Systems programming",
                "Tizimli dasturlash",
            ],
            group: Engineering,
            skills: skills![
                "memory" => ["Память и управление ресурсами", "Memory and resource management", "Xotira va resurslarni boshqarish"],
                "pointers" => ["Указатели и ссылки", "Pointers and references", "Ko‘rsatkichlar va havolalar"],
                "processes-threads" => ["Процессы и потоки", "Processes and threads", "Jarayonlar va oqimlar"],
                "synchronization" => ["Синхронизация", "Synchronisation", "Sinxronlash"],
                "filesystems" => ["Файловые системы", "File systems", "Fayl tizimlari"],
                "system-apis" => ["Системные API", "System APIs", "Tizim API’lari"],
                "abi-ffi" => ["ABI и FFI", "ABI and FFI", "ABI va FFI"],
                "low-level-diagnostics" => ["Низкоуровневая диагностика", "Low-level diagnostics", "Past darajadagi diagnostika"],
            ],
        },
        Discipline {
            id: "computer-graphics",
            name: [
                "Компьютерная графика",
                "Computer graphics",
                "Kompyuter grafikasi",
            ],
            group: Engineering,
            skills: skills![
                "coordinate-systems" => ["Системы координат", "Coordinate systems", "Koordinata tizimlari"],
                "vector-matrix" => ["Векторная и матричная геометрия", "Vector and matrix geometry", "Vektor va matritsa geometriyasi"],
                "affine-transforms" => ["Аффинные преобразования", "Affine transformations", "Affin almashtirishlar"],
                "projections" => ["Проекции", "Projections", "Proyeksiyalar"],
                "rasterization" => ["Растеризация", "Rasterisation", "Rasterlash"],
                "curves-surfaces" => ["Кривые и поверхности", "Curves and surfaces", "Egri chiziqlar va sirtlar"],
                "shaders" => ["Шейдеры", "Shaders", "Sheyderlar"],
                "gpu-pipeline" => ["GPU pipeline", "GPU pipeline", "GPU konveyeri"],
                "rendering" => ["Рендеринг", "Rendering", "Rendering"],
                "spatial-structures" => ["Пространственные структуры данных", "Spatial data structures", "Fazoviy ma’lumotlar tuzilmalari"],
            ],
        },
        Discipline {
            id: "web-ui",
            name: [
                "Вёрстка и интерфейсы",
                "Layout and interfaces",
                "Sahifalash va interfeyslar",
            ],
            group: Engineering,
            skills: skills![
                "html" => ["HTML", "HTML", "HTML"],
                "css" => ["CSS", "CSS", "CSS"],
                "adaptive-layout" => ["Адаптивная вёрстка", "Adaptive layout", "Moslashuvchan sahifalash"],
                "layout-systems" => ["Layout systems", "Layout systems", "Joylashtirish tizimlari"],
                "typography" => ["Типографика", "Typography", "Tipografika"],
                "components" => ["Композиция компонентов", "Component composition", "Komponentlar kompozitsiyasi"],
                "accessibility" => ["Доступность", "Accessibility", "Foydalanish qulayligi"],
                "ui-ux" => ["Основы UI/UX", "UI/UX basics", "UI/UX asoslari"],
                "responsive" => ["Responsive design", "Responsive design", "Responsive dizayn"],
                "frontend-api" => ["Взаимодействие frontend с API", "Frontend–API interaction", "Frontend va API o‘zaro ishlashi"],
            ],
        },
        Discipline {
            id: "architecture",
            name: [
                "Архитектура ПО",
                "Software architecture",
                "Dasturiy ta’minot arxitekturasi",
            ],
            group: Engineering,
            skills: skills![
                "decomposition" => ["Декомпозиция систем", "System decomposition", "Tizimni bo‘laklarga ajratish"],
                "modularity" => ["Модульность", "Modularity", "Modullilik"],
                "api-design" => ["Проектирование API", "API design", "API loyihalash"],
                "dependencies" => ["Управление зависимостями", "Dependency management", "Bog‘liqliklarni boshqarish"],
                "patterns" => ["Паттерны", "Patterns", "Patternlar"],
                "module-boundaries" => ["Границы модулей", "Module boundaries", "Modul chegaralari"],
                "scalability" => ["Масштабируемость", "Scalability", "Masshtablanuvchanlik"],
                "maintainability" => ["Сопровождаемость", "Maintainability", "Qo‘llab-quvvatlanuvchanlik"],
                "tradeoffs" => ["Архитектурные компромиссы", "Architectural trade-offs", "Arxitekturaviy murosalar"],
            ],
        },
        Discipline {
            id: "debugging-testing",
            name: [
                "Отладка и тестирование",
                "Debugging and testing",
                "Debugging va testlash",
            ],
            group: Engineering,
            skills: skills![
                "reproduction" => ["Воспроизведение ошибок", "Reproducing bugs", "Xatolarni takrorlash"],
                "hypotheses" => ["Построение гипотез", "Forming hypotheses", "Gipotezalar tuzish"],
                "debugger" => ["Debugger", "Debugger", "Debugger"],
                "unit-integration" => ["Unit и integration tests", "Unit and integration tests", "Unit va integratsion testlar"],
                "property-based" => ["Property-based testing", "Property-based testing", "Property-based testlash"],
                "edge-cases" => ["Граничные случаи", "Edge cases", "Chegaraviy holatlar"],
                "profiling" => ["Профилирование", "Profiling", "Profiling"],
                "regression" => ["Регрессионное тестирование", "Regression testing", "Regressiya testlash"],
            ],
        },
        Discipline {
            id: "git",
            name: [
                "Инженерная работа с Git",
                "Engineering with Git",
                "Git bilan muhandislik ishi",
            ],
            group: Engineering,
            skills: skills![
                "commits" => ["Коммиты", "Commits", "Kommitlar"],
                "branching" => ["Ветвление", "Branching", "Tarmoqlash"],
                "merge-rebase" => ["Merge и rebase", "Merge and rebase", "Merge va rebase"],
                "conflicts" => ["Разрешение конфликтов", "Resolving conflicts", "Konfliktlarni hal qilish"],
                "diff-analysis" => ["Анализ diff", "Diff analysis", "Diff tahlili"],
                "code-review" => ["Code review", "Code review", "Code review"],
                "pull-requests" => ["Pull Request", "Pull requests", "Pull Request"],
                "issues" => ["Issues", "Issues", "Issues"],
                "history" => ["История изменений", "Change history", "O‘zgarishlar tarixi"],
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
pub fn matrix(lang: &str, practice: impl Fn(&str) -> u32) -> Vec<MatrixDiscipline> {
    catalog()
        .into_iter()
        .map(|d| MatrixDiscipline {
            id: d.id,
            name: name_in(&d.name, lang),
            group: d.group,
            status: AssessmentStatus::NotAssessed,
            score: None,
            practice_tasks: practice(d.id),
            skills: d
                .skills
                .into_iter()
                .map(|s| MatrixSkill {
                    id: s.id,
                    name: name_in(&s.name, lang),
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
        let m = matrix("en", |_| 0);
        assert_eq!(m[0].name, "Algorithmic thinking");
        assert_eq!(matrix("uz", |_| 0)[9].skills[0].name, "Kommitlar");
        assert!(
            m.iter()
                .all(|d| d.score.is_none() && d.status == AssessmentStatus::NotAssessed)
        );
        assert!(m.iter().flat_map(|d| &d.skills).all(|s| s.score.is_none()));
        assert!(is_known_discipline("debugging-testing") && !is_known_discipline("magic"));
    }
}
