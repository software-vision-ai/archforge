# archforge-mcp

**Purpose.** The MCP server agents use (agent-control-surfaces §10.5, BP-CS-4): JSON-RPC 2.0 over the
standard streams, one message per line, protocol versions 2024-11-05, 2025-03-26 and 2025-06-18,
bridged to the running Archforge's control channel. Tools: `list_commands`, `describe_command`,
`execute_command`, `batch`, `inspect_document`, `undo`, `redo`, `call_app`; only the reads carry
`readOnlyHint`.

**Layer.** 9, above `control`, which it reaches the application through.

**Public API.**

| Item | Signature |
|---|---|
| `serve` | `pub fn serve(input: &mut impl BufRead, output: &mut impl Write, backend: &dyn Backend) -> io::Result<()>` |
| `handle` | `pub fn handle(line: &[u8], backend: &dyn Backend) -> Option<Value>` (none for a notification) |
| `Backend` | `fn call(&self, method: &str, params: Value) -> Result<Value, CallError>`; `archforge_control::Client` is one |
| `tool_list`, `tool_methods`, `VERSIONS` | the tools as `tools/list` gives them; the control methods they call |

**Non-goals.** The headless mode with its own engine: not built, not advertised.
