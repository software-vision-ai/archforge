//! The OpenAI Codex adapter (AR-202, after the AR-196 spike in `docs/agent-runtimes/README.md`).
//! Codex's threads are ephemeral, so Archforge keeps each conversation's messages itself, in the
//! conversation's copy ([`history`]).

pub mod history;
