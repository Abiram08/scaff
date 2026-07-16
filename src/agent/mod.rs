//! Pi-style Harness research agent: one loop, four tools, short prompt.

pub mod r#loop;
pub mod prompt;
pub mod tools;
pub mod types;

pub use r#loop::run;
pub use types::{
    AgentOptions, AgentResult, AgentStats, Confidence, Finding, FinishPayload, SourceRef, UseCase,
};
