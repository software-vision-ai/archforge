# Skills

A skill is guidance written on the model: how tables are named, which SQL dialect the project
uses, what a class's operations must not do. People read it, and an agent reads the skills that
apply to the element it works on.

## Where skills live

A skill sits on the whole project or on one element: a schema, a table, a package, a class, an
interface, an enumeration. A skill on a container applies to everything inside it, and a skill on
an element overrides one of the same name further out: a schema's *Naming* skill wins over the
project's *Naming* skill for that schema's tables.

## Writing a skill

1. Select the element, or nothing for the project, and open **Skills** from the right rail.
2. Choose **+**. The new skill opens in its own tab in the centre, its name selected: type a name.
3. Write the text in Markdown. It starts from a template with a short header, a title, what the
   skill is for, then *Rules* and *Examples* to write over.

<!-- picture: workspace-skill-tab light | A skill's tab: its name, where it sits, the formatting toolbar and the text -->

The toolbar formats the text (bold, italic, heading, lists, code, link, quote), and the pencil and
eye switch between writing and a preview. The text is kept a moment after you stop typing, and
each keep is one undo step.

**Skills** in the right panel lists the skills that apply to the selection, with where each comes
from; choose one to open it, or hover over one of the element's own skills to remove it.

<!-- picture: workspace-skills light | The Skills panel lists the skills on the selection and those it inherits -->

Links in a skill's preview open only after Archforge shows you the whole address and you confirm.
Pictures in a skill are never loaded.

Next: [Working with an agent](07-agents.md).
