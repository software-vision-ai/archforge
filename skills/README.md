# Agent skills

Skills for working on Archforge. Each folder is one skill in the [Agent Skills](https://agentskills.io)
format: a `SKILL.md` whose name and description tell an agent when to load it. `AGENTS.md` holds the
rules in one line each and routes to these files for the detail; people can read them the same way.

| Skill | Load it when you |
|---|---|
| [`archforge-ground-rules`](archforge-ground-rules/SKILL.md) | add a dependency, asset or wording, or are unsure what may be copied or named |
| [`archforge-crash-safety`](archforge-crash-safety/SKILL.md) | write non-test Rust, read input, write a file or look into a crash |
| [`archforge-design`](archforge-design/SKILL.md) | design or change how a screen, tool, editor, notation, message or icon looks or behaves |
| [`archforge-ui`](archforge-ui/SKILL.md) | change what the interface draws or says, or add a translation |
| [`archforge-tests`](archforge-tests/SKILL.md) | write, place or run tests, or the gate fails |

## Keeping them true

`cargo xtask skills`, a step of the gate, fails when this table, the table in `AGENTS.md` and the
folders disagree, when a skill's name is not its folder's, or when a skill or `AGENTS.md` names a
file, a `cargo xtask` verb or a skill that is not there. A change that moves a file or renames a
verb these files name updates them in the same change.
