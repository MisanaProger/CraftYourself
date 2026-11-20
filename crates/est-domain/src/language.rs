use serde::{Deserialize, Serialize};

/// A language found in a project. A project may contain several languages;
/// none of them restricts what EST can do with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageDetection {
    /// Adapter id, e.g. `rust`, `cpp`.
    pub language: String,
    /// Number of tracked source files in this language.
    pub files: u32,
    /// Build system markers found, e.g. `cargo`, `cmake`.
    pub build_systems: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Build,
    Test,
    Check,
}

/// Description of a command. Language adapters only *describe* commands;
/// running them is the Sandbox Manager's job (architecture v0.2, §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub kind: CommandKind,
    pub language: String,
    pub program: String,
    pub args: Vec<String>,
    /// Working directory relative to the workspace root.
    pub workdir: String,
    pub description: String,
}

impl CommandSpec {
    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
