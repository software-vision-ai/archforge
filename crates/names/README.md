# archforge-names

**Purpose.** The rules for a name that becomes a file name, such as a project's: one set, shared by
the interface (to explain a name as it is typed) and the host (to refuse one), so they never
disagree.

**Layer.** Standalone: no workspace dependencies, pure, builds for the portable target.

**Public API.**

| Item | Signature |
|---|---|
| `problem` | `pub fn problem(name: &str) -> Option<Problem>` (the first rule a name breaks) |
| `Problem` | Empty, ControlCharacter, Forbidden(char), TooLong { max_bytes }, EdgeSpace, LeadingDot, TrailingDot, Reserved |
| `MAX_BYTES`, `FORBIDDEN`, `EXTENSION` | 245 bytes, the characters no platform takes, `archforge` |

**Non-goals.** Translating the rules: the interface says each one in its own words.
