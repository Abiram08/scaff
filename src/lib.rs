//! Library facade for the `scaff` binary. Exposes modules for integration tests
//! and for embedding the research agent in other tools (including the MCP server).

pub mod cli;
pub mod config;
pub mod connectors;
pub mod corpus;
pub mod display;
pub mod history;
pub mod init;
pub mod llm;
pub mod local_files;
pub mod mcp;
pub mod pipeline;
pub mod render;
pub mod repl;
pub mod research;
pub mod search;
pub mod web;
