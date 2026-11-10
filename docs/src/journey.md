# User journeys

sirene draws Mermaid user journeys (`journey`) as images and as text:

```
$ sirene ascii day.mmd
         My working day

Go to work
  Make tea       ●●●●● 5  Me
  Go upstairs    ●●●○○ 3  Me
  Do work        ●○○○○ 1  Me, Cat
Go home
  Go downstairs  ●●●●● 5  Me
  Sit down       ●●●●● 5  Me
```

```
journey
    title My working day
    section Go to work
      Make tea: 5: Me
      Go upstairs: 3: Me
      Do work: 1: Me, Cat
    section Go home
      Go downstairs: 5: Me
      Sit down: 5: Me
```

In the editor a user journey is read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `title text` | the title above; frontmatter `title` too |
| `section name` | a colored box over its tasks |
| `name: score: actor, actor` | a task box with a dot per actor, and a face below it for the score |
| `accTitle`, `accDescr` | kept in the model |

A score runs from 1 to 5 and may have a fraction. Faces sit lower for lower scores, and smile
above 3, frown below 3, and are level at 3. Actors are listed with their colors at the left. In
text, each task has a bar of five steps for its score, rounded.

## Errors

A task with no score or a score that is not a number is an error, and the task is skipped. A
score outside 1 to 5 is a warning and is drawn at the nearest end. Text after the actors (`5: Me:
more`) is a warning and ignored.
