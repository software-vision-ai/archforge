# Archforge user manual

Archforge is a software modelling tool: a desktop application where people and AI agents work on
the same model of a software system together. This manual takes you from opening Archforge for
the first time to a small model you can build on, and shows how an agent joins in.

## Chapters

1. [What Archforge is](01-introduction.md): what it does today, what is planned, and the words
   this manual uses.
2. [Getting started](02-getting-started.md): running Archforge, and creating, saving, reopening
   and finding a project.
3. [The workspace](03-workspace.md): Diagrams and Models, Properties, search, undo, and removing
   versus deleting.
4. [A first ER model](04-er-model.md): the database aspect of a system, with tables, columns,
   keys and a relationship.
5. [A first UML class model](05-uml-class-model.md): the classes of a system's code. One
   walkthrough from a first class to interfaces, enumerations, data types, primitive types,
   association classes, property modifiers and constraints, with a guide to which element you
   need. Then [Going further](05-uml-class-model.md#going-further), short sections to read when
   you need them:
   - [static members](05-uml-class-model.md#a-member-of-the-class-as-a-whole-static-members);
   - [qualified associations](05-uml-class-model.md#find-an-item-by-a-key-qualified-associations)
     and [n-ary associations](05-uml-class-model.md#link-three-things-at-once-n-ary-associations);
   - [generalization sets](05-uml-class-model.md#divide-a-class-into-kinds-generalization-sets),
     [final](05-uml-class-model.md#allow-no-more-specific-kinds-final) and
     [leaf](05-uml-class-model.md#allow-no-replaced-members-leaf);
   - [nested classifiers](05-uml-class-model.md#keep-a-type-inside-its-class-nested-classifiers);
   - [defining a template](05-uml-class-model.md#one-class-for-many-types-defining-a-template) and
     [binding it](05-uml-class-model.md#fill-in-a-template-binding-it);
   - [defining a profile](05-uml-class-model.md#words-of-your-own-defining-a-profile-and-its-stereotypes)
     and [applying its stereotypes](05-uml-class-model.md#label-elements-with-your-words-applying-stereotypes).

   A reference of the rules closes the chapter.
6. [Skills](06-skills.md): guidance written on the model for people and agents.
7. [Working with an agent](07-agents.md): letting an agent read and change the model, and the
   Generate handoff.
8. [Help and support](08-help-and-support.md): About Archforge, supporting the project, and
   reporting a problem.

## Keys

Shortcuts are written for macOS, with Windows and Linux in brackets: ⌘K (Ctrl K) means the command
key and K on a Mac, the control key and K elsewhere.

## Pictures

The pictures are drawn by Archforge itself, from the same code as the window, so they always show
the current application. They are not stored in the repository. To see the manual with its
pictures, build it:

```sh
cargo xtask manual
```

The illustrated chapters and their pictures are written to `plan/manual/`; open
`plan/manual/README.md` there. In the text here, each picture is a comment that names the screen it
shows.

The sections of [Going further](05-uml-class-model.md#going-further) show their pictures here too:
each release carries them, drawn from its own code (`cargo xtask screens --gallery`), and the
chapter loads them from the latest release. Until a release has a section's feature, its picture
does not load.

## Keeping the manual current

The manual describes what Archforge does now, never what it may do later. Whoever changes a flow a
user sees (a screen, a dialog, a tool, a shortcut or a message) updates the chapter that describes
it in the same change, and checks the steps still work in the running application. A new feature
gets its section in the chapter it belongs to, or a new numbered chapter after the last. The
maintainers review the manual with every change that touches the interface; `cargo test -p xtask`
checks that every picture names a screen that exists and every link leads somewhere.
