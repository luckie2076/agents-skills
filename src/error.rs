//! Unified error types.

#[derive(Debug, thiserror::Error)]
pub enum SkillsError {
    /// A user-facing plain error message.
    #[error("{0}")]
    Message(String),
    /// One or more invalid agent names (comma-separated).
    #[error("invalid agents: {0}")]
    InvalidAgents(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Http(#[from] ureq::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
}

/// Project-wide unified `Result` alias.
pub type Result<T> = std::result::Result<T, SkillsError>;

/// Semantic alias for library consumers.
pub type Error = SkillsError;

impl SkillsError {
    pub fn msg(msg: impl Into<String>) -> Self {
        SkillsError::Message(msg.into())
    }
}
