//! Engagement Controller domain (architecture v0.3, §7).
//!
//! The engagement level bounds how much help the AI may give in the current
//! Issue. It rises only by explicit consent, one step at a time, and every
//! new Issue starts at L0.

use est_core::{ConversationId, EscalationId, MessageId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum EngagementLevel {
    /// L0 — Socratic: questions only, the solution is not revealed.
    Socratic = 0,
    /// L1 — Hinting: small hints, where to look, sources.
    Hinting = 1,
    /// L2 — Explaining: concepts, diagnosis, causes of errors.
    Explaining = 2,
    /// L3 — Demonstration: pseudocode, small local code fragments.
    Demonstration = 3,
    /// L4 — Active: detailed solution and significant code.
    Active = 4,
}

impl EngagementLevel {
    pub const ALL: [EngagementLevel; 5] = [
        EngagementLevel::Socratic,
        EngagementLevel::Hinting,
        EngagementLevel::Explaining,
        EngagementLevel::Demonstration,
        EngagementLevel::Active,
    ];

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// The next level, if any. Escalation is always a single step.
    pub fn next(self) -> Option<Self> {
        Self::try_from(self.as_u8() + 1).ok()
    }

    pub fn code(self) -> &'static str {
        ["L0", "L1", "L2", "L3", "L4"][self as usize]
    }

    pub fn name_ru(self) -> &'static str {
        match self {
            EngagementLevel::Socratic => "Сократический",
            EngagementLevel::Hinting => "Наводящий",
            EngagementLevel::Explaining => "Объясняющий",
            EngagementLevel::Demonstration => "Демонстрационный",
            EngagementLevel::Active => "Активный",
        }
    }

    /// What becomes available at this level, shown in escalation proposals.
    pub fn unlocks_ru(self) -> &'static str {
        match self {
            EngagementLevel::Socratic => "вопросы, которые помогают рассуждать самостоятельно",
            EngagementLevel::Hinting => "небольшие подсказки, направления поиска и источники",
            EngagementLevel::Explaining => {
                "объяснение концепций, диагностику и разбор причин ошибки"
            }
            EngagementLevel::Demonstration => "псевдокод и небольшие фрагменты реализации",
            EngagementLevel::Active => "подробное решение и значительные фрагменты кода",
        }
    }
}

impl TryFrom<u8> for EngagementLevel {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Self::ALL
            .get(usize::from(v))
            .copied()
            .ok_or_else(|| format!("engagement level must be in 0..=4, got {v}"))
    }
}

impl From<EngagementLevel> for u8 {
    fn from(l: EngagementLevel) -> u8 {
        l.as_u8()
    }
}

/// Current engagement of one conversation (one Issue).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngagementState {
    pub conversation_id: ConversationId,
    pub level: EngagementLevel,
    /// Upper bound set by the task author.
    pub max_level: EngagementLevel,
    pub updated_at: Timestamp,
}

impl EngagementState {
    /// Every new Issue starts at L0 (v0.3 §7.3, fixed decision).
    pub fn initial(conversation_id: ConversationId, max_level: EngagementLevel) -> Self {
        Self {
            conversation_id,
            level: EngagementLevel::Socratic,
            max_level,
            updated_at: est_core::now(),
        }
    }

    /// The single step to propose when a request needs `required`, if any.
    pub fn escalation_target(&self, required: EngagementLevel) -> Option<EngagementLevel> {
        if required <= self.level {
            return None;
        }
        self.level.next().filter(|next| *next <= self.max_level)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationStatus {
    Proposed,
    Accepted,
    Rejected,
}

/// A proposal to raise the level by one step; never applied without consent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Escalation {
    pub id: EscalationId,
    pub conversation_id: ConversationId,
    pub from: EngagementLevel,
    pub to: EngagementLevel,
    /// The level the blocked request needed.
    pub required: EngagementLevel,
    /// The user message that triggered the proposal.
    pub message_id: MessageId,
    pub status: EscalationStatus,
    pub created_at: Timestamp,
    pub resolved_at: Option<Timestamp>,
}

impl Escalation {
    pub fn propose(
        state: &EngagementState,
        to: EngagementLevel,
        required: EngagementLevel,
        message_id: MessageId,
    ) -> Result<Self, DomainError> {
        if Some(to) != state.level.next() || to > state.max_level {
            return Err(DomainError::Invalid(format!(
                "escalation must go one step from {} and stay within {}",
                state.level.code(),
                state.max_level.code()
            )));
        }
        Ok(Self {
            id: EscalationId::generate(),
            conversation_id: state.conversation_id.clone(),
            from: state.level,
            to,
            required,
            message_id,
            status: EscalationStatus::Proposed,
            created_at: est_core::now(),
            resolved_at: None,
        })
    }

    /// Applies consent. Fails if already resolved or if the level changed
    /// since the proposal (stale proposals are never applied).
    pub fn accept(&mut self, state: &mut EngagementState) -> Result<(), DomainError> {
        self.ensure_open()?;
        if state.level != self.from {
            return Err(DomainError::InvalidTransition(
                "the engagement level changed since this proposal".into(),
            ));
        }
        self.status = EscalationStatus::Accepted;
        self.resolved_at = Some(est_core::now());
        state.level = self.to;
        state.updated_at = est_core::now();
        Ok(())
    }

    pub fn reject(&mut self) -> Result<(), DomainError> {
        self.ensure_open()?;
        self.status = EscalationStatus::Rejected;
        self.resolved_at = Some(est_core::now());
        Ok(())
    }

    fn ensure_open(&self) -> Result<(), DomainError> {
        if self.status == EscalationStatus::Proposed {
            Ok(())
        } else {
            Err(DomainError::InvalidTransition(
                "escalation is already resolved".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EngagementLevel::*;

    fn state(level: EngagementLevel, max: EngagementLevel) -> EngagementState {
        EngagementState {
            level,
            ..EngagementState::initial(ConversationId::from("c"), max)
        }
    }

    #[test]
    fn new_issue_starts_at_l0() {
        assert_eq!(
            EngagementState::initial(ConversationId::from("c"), Active).level,
            Socratic
        );
    }

    #[test]
    fn escalation_is_one_step() {
        let s = state(Socratic, Active);
        assert_eq!(s.escalation_target(Active), Some(Hinting));
        assert_eq!(s.escalation_target(Socratic), None);
        assert_eq!(
            state(Explaining, Explaining).escalation_target(Active),
            None,
            "capped by task"
        );
        assert!(Escalation::propose(&s, Explaining, Active, MessageId::from("m")).is_err());
    }

    #[test]
    fn accept_and_reject() {
        let mut s = state(Socratic, Active);
        let mut e = Escalation::propose(&s, Hinting, Demonstration, MessageId::from("m")).unwrap();
        e.accept(&mut s).unwrap();
        assert_eq!(s.level, Hinting);
        assert!(e.accept(&mut s).is_err(), "consent is not reusable");
        assert!(e.reject().is_err());

        let mut stale = Escalation::propose(&s, Explaining, Active, MessageId::from("m2")).unwrap();
        s.level = Explaining;
        assert!(stale.accept(&mut s).is_err(), "stale proposal");
        stale.reject().unwrap();
        assert_eq!(stale.status, EscalationStatus::Rejected);
    }

    #[test]
    fn serde_as_number() {
        assert_eq!(serde_json::to_string(&Demonstration).unwrap(), "3");
        assert!(serde_json::from_str::<EngagementLevel>("5").is_err());
    }
}
