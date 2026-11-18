# Glossary Format

Bowties uses a single glossary at `product/glossary.md`. Update it inline during grilling sessions as terms are resolved.

## Structure

```md
# Bowties Glossary

Domain vocabulary for humans and AI agents working in the Bowties codebase. Canonical terms are **bolded**; alternatives to avoid are listed under _Avoid_.

## {Group Name}

**Term**:
A concise description of what this term IS.
_Avoid_: synonym1, synonym2

**Another Term**:
One sentence defining the concept.
_Avoid_: confusing alternative

## Relationships

- A **Bowtie** contains one or more **Connectors**
- A **Connector** has exactly two **Pills**

## Flagged ambiguities

- "node" was used to mean both **Node** (LCC network participant) and DOM node — resolved: always capitalize when referring to LCC nodes.
```

## Rules

- **Be opinionated.** When multiple words exist for the same concept, pick the best one and list the others as aliases to avoid.
- **Flag conflicts explicitly.** If a term is used ambiguously, call it out in "Flagged ambiguities" with a clear resolution.
- **Keep definitions tight.** One sentence max. Define what it IS, not what it does.
- **Show relationships.** Use bold term names and express cardinality where obvious.
- **Only include domain terms.** General programming concepts (timeouts, error types, utility patterns) don't belong. Before adding a term, ask: is this a concept specific to Bowties, LCC/OpenLCB, or the problem domain? Only domain-specific terms belong.
- **Group terms under subheadings** when natural clusters emerge (e.g., Protocol, App model, Architecture roles).
- **Include _Avoid_ lists** for every term where confusion is likely. These prevent AI agents from drifting into ambiguous synonyms.