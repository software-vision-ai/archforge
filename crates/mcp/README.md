# archforge-mcp

**Purpose.** The MCP server agents use (agent-control-surfaces §10.5, BP-CS-4): JSON-RPC 2.0 over the
standard streams, one message per line, protocol versions 2024-11-05, 2025-03-26 and 2025-06-18,
bridged to the running Archforge's control channel. Tools: `list_commands`, `describe_command`,
`execute_command`, `batch`, `inspect_document`, `undo`, `redo`, `call_app`, and one tool per command
of the table (`er_entity_add` for `er.entity.add`), its schema generated from the command's
parameter documentation (types, required keys, choices, ranges, defaults, and `coalesce` on undoable
commands); `edit.undo` and `edit.redo` are left to `undo` and `redo`, and a name another tool has is
never shadowed. `compact` lists only the general tools and `search_tools`, whose finds run through
`execute_command` (CS-14). Only the reads carry `readOnlyHint`; a change of the view alone is not
destructive.

**Layer.** 9, above `control`, which it reaches the application through.

**Public API.**

| Item | Signature |
|---|---|
| `serve` | `pub fn serve(input: &mut impl BufRead, output: &mut impl Write, backend: &dyn Backend, catalog: &Catalog) -> io::Result<()>` |
| `handle` | `pub fn handle(line: &[u8], backend: &dyn Backend, catalog: &Catalog) -> Option<Value>` (none for a notification) |
| `Catalog::new`, `Catalog::list` | the tools for the commands the engine's `CommandInfo`s describe, compact or not; `tools/list`'s answer |
| `tool_name` | a command id as a tool name: `er.foreign-key.add` → `er_foreign_key_add` |
| `Backend` | `fn call(&self, method: &str, params: Value) -> Result<Value, CallError>`; `archforge_control::Client` is one |
| `tool_list`, `tool_methods`, `VERSIONS` | the tools as `tools/list` gives them; the control methods they call |

**Non-goals.** The headless mode with its own engine: not built, not advertised.
