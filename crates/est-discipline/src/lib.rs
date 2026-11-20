//! Engineering Skill Matrix catalog (architecture v0.3, §11).
//!
//! `Engineering Profile → Discipline → Skill → Assessment Evidence`.
//! The catalog is data: new disciplines are added here without touching the
//! core. Absence of evidence is reported as `not_assessed`, never as zero.

use std::collections::{BTreeMap, BTreeSet};

use est_core::Timestamp;
use serde::{Deserialize, Serialize};

/// Version of the catalog and its criteria; stored with every assessment.
pub const CATALOG_VERSION: &str = "esm-catalog-0.2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisciplineGroup {
    /// A. Fundamental disciplines.
    Fundamental,
    /// B. Engineering disciplines.
    Engineering,
}

/// Dimensions a skill can be assessed on (§11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Theory,
    Practice,
    Diagnosis,
    Transfer,
    Independence,
}

impl Dimension {
    pub const ALL: [Dimension; 5] = [
        Dimension::Theory,
        Dimension::Practice,
        Dimension::Diagnosis,
        Dimension::Transfer,
        Dimension::Independence,
    ];
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

/// Whether `skill` belongs to `discipline` in the catalog.
pub fn is_known_skill(discipline: &str, skill: &str) -> bool {
    catalog()
        .iter()
        .any(|d| d.id == discipline && d.skills.iter().any(|s| s.id == skill))
}

// ---- assessment evidence (§11.4) -------------------------------------------

/// Where a piece of evidence was observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    /// One graded quiz attempt.
    Quiz,
    /// A result reported by an interactive artifact.
    Artifact,
    /// A practice project of a learning program, completed.
    Practice,
    /// A completed Issue.
    Issue,
}

impl EvidenceSource {
    /// Work on real code weighs more than a quiz or a trainer.
    pub fn weight(self) -> u32 {
        match self {
            EvidenceSource::Quiz | EvidenceSource::Artifact => 1,
            EvidenceSource::Practice | EvidenceSource::Issue => 2,
        }
    }
}

/// One observed result, 0–100, for a discipline or one of its skills.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub discipline: String,
    /// `None`: evidence for the discipline as a whole.
    pub skill: Option<String>,
    pub source: EvidenceSource,
    pub dimension: Dimension,
    pub score: u8,
    /// What was observed: `program/lesson` or a session id. A newer result
    /// for the same reference and dimension replaces the older one in the
    /// score; both stay in the history.
    pub reference: String,
    pub title: String,
    pub observed_at: Timestamp,
}

/// Evidence older than this needs reassessment.
pub const STALE_AFTER_DAYS: i64 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DimensionScore {
    pub dimension: Dimension,
    pub score: u8,
    pub evidence_count: u32,
}

/// The assessment of a skill or discipline from its evidence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Assessment {
    pub status: AssessmentStatus,
    /// `None` without evidence: no data is not a zero.
    pub score: Option<u8>,
    pub confidence: Option<Confidence>,
    /// Distinct things observed (references).
    pub evidence_count: u32,
    pub dimensions: Vec<DimensionScore>,
    pub last_observed: Option<Timestamp>,
}

fn weighted(items: &[&Evidence]) -> Option<u8> {
    let w: u32 = items.iter().map(|e| e.source.weight()).sum();
    (w > 0).then(|| {
        let sum: u32 = items
            .iter()
            .map(|e| u32::from(e.score.min(100)) * e.source.weight())
            .sum();
        ((sum + w / 2) / w) as u8
    })
}

/// Rules (catalog `esm-catalog-0.2`):
/// - the latest result per reference and dimension counts; the score is
///   their mean weighted by source (practice and Issues ×2);
/// - no evidence → `not_assessed`; one reference → `insufficient_evidence`;
///   nothing newer than [`STALE_AFTER_DAYS`] → `needs_reassessment`;
///   at least three references from at least two sources or dimensions →
///   `established`; otherwise `provisional`;
/// - confidence: 1–2 references low, 3–5 medium, 6+ high.
pub fn assess(evidence: &[&Evidence], now: Timestamp) -> Assessment {
    let mut latest: BTreeMap<(&str, Dimension), &Evidence> = BTreeMap::new();
    for e in evidence {
        let slot = latest
            .entry((e.reference.as_str(), e.dimension))
            .or_insert(e);
        if e.observed_at >= slot.observed_at {
            *slot = e;
        }
    }
    let items: Vec<&Evidence> = latest.into_values().collect();
    let references: BTreeSet<&str> = items.iter().map(|e| e.reference.as_str()).collect();
    let n = references.len() as u32;
    let last_observed = items.iter().map(|e| e.observed_at).max();
    let kinds: BTreeSet<String> = items
        .iter()
        .flat_map(|e| [format!("s:{:?}", e.source), format!("d:{:?}", e.dimension)])
        .collect();
    let varied = kinds.iter().filter(|k| k.starts_with("s:")).count() > 1
        || kinds.iter().filter(|k| k.starts_with("d:")).count() > 1;
    let status = match (n, last_observed) {
        (0, _) | (_, None) => AssessmentStatus::NotAssessed,
        (_, Some(t)) if (now - t).num_days() > STALE_AFTER_DAYS => {
            AssessmentStatus::NeedsReassessment
        }
        (1, _) => AssessmentStatus::InsufficientEvidence,
        (n, _) if n >= 3 && varied => AssessmentStatus::Established,
        _ => AssessmentStatus::Provisional,
    };
    let mut dimensions = Vec::new();
    for d in Dimension::ALL {
        let of: Vec<&Evidence> = items.iter().copied().filter(|e| e.dimension == d).collect();
        if let Some(score) = weighted(&of) {
            dimensions.push(DimensionScore {
                dimension: d,
                score,
                evidence_count: of.len() as u32,
            });
        }
    }
    Assessment {
        status,
        score: weighted(&items),
        confidence: match n {
            0 => None,
            1..=2 => Some(Confidence::Low),
            3..=5 => Some(Confidence::Medium),
            _ => Some(Confidence::High),
        },
        evidence_count: n,
        dimensions,
        last_observed,
    }
}

/// One row of the matrix as shown to the learner.
#[derive(Debug, Clone, Serialize)]
pub struct MatrixSkill {
    pub id: &'static str,
    pub name: &'static str,
    #[serde(flatten)]
    pub assessment: Assessment,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatrixDiscipline {
    pub id: &'static str,
    pub name: &'static str,
    pub group: DisciplineGroup,
    /// All evidence of the discipline: its skills' and its own.
    #[serde(flatten)]
    pub assessment: Assessment,
    pub skills: Vec<MatrixSkill>,
    /// Completed tasks that exercised this discipline.
    pub practice_tasks: u32,
    /// Evidence history, newest first (at most [`HISTORY`] items).
    pub history: Vec<Evidence>,
}

pub const HISTORY: usize = 30;

/// Builds the matrix from evidence. Evidence for an unknown discipline is
/// ignored; evidence for an unknown skill counts for its discipline only.
pub fn matrix(
    lang: &str,
    practice: impl Fn(&str) -> u32,
    evidence: &[Evidence],
    now: Timestamp,
) -> Vec<MatrixDiscipline> {
    catalog()
        .into_iter()
        .map(|d| {
            let own: Vec<&Evidence> = evidence.iter().filter(|e| e.discipline == d.id).collect();
            let mut history: Vec<Evidence> = own.iter().map(|e| (*e).clone()).collect();
            history.sort_by_key(|e| std::cmp::Reverse(e.observed_at));
            history.truncate(HISTORY);
            MatrixDiscipline {
                id: d.id,
                name: name_in(&d.name, lang),
                group: d.group,
                assessment: assess(&own, now),
                practice_tasks: practice(d.id),
                skills: d
                    .skills
                    .iter()
                    .map(|s| {
                        let of: Vec<&Evidence> = own
                            .iter()
                            .copied()
                            .filter(|e| e.skill.as_deref() == Some(s.id))
                            .collect();
                        MatrixSkill {
                            id: s.id,
                            name: name_in(&s.name, lang),
                            assessment: assess(&of, now),
                        }
                    })
                    .collect(),
                history,
            }
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
        let m = matrix("en", |_| 0, &[], est_core::now());
        assert_eq!(m[0].name, "Algorithmic thinking");
        assert_eq!(
            matrix("uz", |_| 0, &[], est_core::now())[9].skills[0].name,
            "Kommitlar"
        );
        assert!(m.iter().all(|d| d.assessment.score.is_none()
            && d.assessment.status == AssessmentStatus::NotAssessed));
        assert!(
            m.iter()
                .flat_map(|d| &d.skills)
                .all(|s| s.assessment.score.is_none())
        );
        assert!(is_known_discipline("debugging-testing") && !is_known_discipline("magic"));
        assert!(is_known_skill("debugging-testing", "regression"));
        assert!(!is_known_skill("git", "regression"));
    }

    fn ev(
        skill: Option<&str>,
        source: EvidenceSource,
        dim: Dimension,
        score: u8,
        r: &str,
        days_ago: i64,
    ) -> Evidence {
        Evidence {
            discipline: "debugging-testing".into(),
            skill: skill.map(str::to_owned),
            source,
            dimension: dim,
            score,
            reference: r.into(),
            title: r.into(),
            observed_at: est_core::now() - chrono::Duration::days(days_ago),
        }
    }

    #[test]
    fn assessment_from_evidence() {
        use Dimension::{Independence, Theory};
        use EvidenceSource::{Issue, Quiz};
        let now = est_core::now();
        assert_eq!(assess(&[], now).status, AssessmentStatus::NotAssessed);

        // Retaking a quiz: only the latest attempt counts.
        let a = ev(Some("unit-integration"), Quiz, Theory, 40, "testing/q1", 3);
        let b = ev(Some("unit-integration"), Quiz, Theory, 90, "testing/q1", 1);
        let one = assess(&[&a, &b], now);
        assert_eq!(
            (one.status, one.score, one.evidence_count, one.confidence),
            (
                AssessmentStatus::InsufficientEvidence,
                Some(90),
                1,
                Some(Confidence::Low)
            )
        );

        // Practice weighs twice as much; varied sources establish the skill.
        let c = ev(
            Some("unit-integration"),
            EvidenceSource::Practice,
            Dimension::Practice,
            60,
            "s1",
            1,
        );
        let d = ev(None, Issue, Independence, 100, "s2", 0);
        let all = assess(&[&a, &b, &c, &d], now);
        assert_eq!(all.status, AssessmentStatus::Established);
        assert_eq!(all.score, Some(82)); // (90 + 2*60 + 2*100) / 5
        assert_eq!(all.confidence, Some(Confidence::Medium));
        assert_eq!(
            all.dimensions
                .iter()
                .map(|d| (d.dimension, d.score))
                .collect::<Vec<_>>(),
            [(Theory, 90), (Dimension::Practice, 60), (Independence, 100)]
        );
        let two = assess(&[&b, &c], now);
        assert_eq!(two.status, AssessmentStatus::Provisional);

        // Old evidence needs reassessment.
        let old = ev(None, Quiz, Theory, 100, "testing/q2", STALE_AFTER_DAYS + 2);
        assert_eq!(
            assess(&[&old], now).status,
            AssessmentStatus::NeedsReassessment
        );
    }

    #[test]
    fn matrix_places_evidence() {
        let e = [
            ev(
                Some("regression"),
                EvidenceSource::Practice,
                Dimension::Practice,
                100,
                "s1",
                2,
            ),
            ev(
                Some("no-such-skill"),
                EvidenceSource::Quiz,
                Dimension::Theory,
                50,
                "q",
                1,
            ),
            Evidence {
                discipline: "magic".into(),
                ..ev(None, EvidenceSource::Quiz, Dimension::Theory, 10, "m", 0)
            },
        ];
        let m = matrix("en", |_| 0, &e, est_core::now());
        let dt = m.iter().find(|d| d.id == "debugging-testing").unwrap();
        assert_eq!(dt.assessment.evidence_count, 2);
        assert_eq!(dt.history.len(), 2);
        assert_eq!(dt.history[0].reference, "q", "newest first");
        let regression = dt.skills.iter().find(|s| s.id == "regression").unwrap();
        assert_eq!(regression.assessment.score, Some(100));
        assert!(
            dt.skills
                .iter()
                .filter(|s| s.id != "regression")
                .all(|s| s.assessment.score.is_none())
        );
        assert!(
            m.iter()
                .filter(|d| d.id != "debugging-testing")
                .all(|d| d.history.is_empty())
        );
    }
}
