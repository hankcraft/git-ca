use std::io;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not authenticated — run `git ca auth login`")]
    NotAuthenticated,

    #[error("device flow: {0}")]
    DeviceFlow(String),

    #[error("Copilot token rejected — run `git ca auth login`")]
    CopilotAuth,

    #[error("Copilot rate limited — retry in {retry_after}s")]
    CopilotRateLimited { retry_after: u64 },

    #[error("Copilot API {status}: {body}")]
    CopilotServer { status: u16, body: String },

    #[error("Codex login: {0}")]
    CodexLogin(String),

    #[error("Codex token rejected — run `git ca auth login --provider codex`")]
    CodexAuth,

    #[error("Codex rate limited — retry in {retry_after}s")]
    CodexRateLimited { retry_after: u64 },

    #[error("Codex API {status}: {body}")]
    CodexServer { status: u16, body: String },

    #[error("LLM returned an empty message")]
    EmptyModelResponse,

    #[error("nothing staged — use `git add` first")]
    NoStagedChanges,

    #[error("not a git repository — run git-ca from inside a Git working tree")]
    NotGitRepository,

    #[error("git {0} exited with status {1}")]
    Git(String, i32),

    #[error(transparent)]
    Network(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error("config: {0}")]
    Config(String),
}

impl Error {
    /// Exit code mapped to error variant. See plan for the table.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::NotAuthenticated
            | Error::DeviceFlow(_)
            | Error::CopilotAuth
            | Error::CodexLogin(_)
            | Error::CodexAuth => 2,
            Error::CopilotRateLimited { .. }
            | Error::CopilotServer { .. }
            | Error::CodexRateLimited { .. }
            | Error::CodexServer { .. }
            | Error::Network(_)
            | Error::EmptyModelResponse => 3,
            Error::NoStagedChanges | Error::NotGitRepository => 1,
            Error::Git(_, code) => *code,
            Error::Io(_) | Error::Serde(_) | Error::Config(_) => 1,
        }
    }
}
