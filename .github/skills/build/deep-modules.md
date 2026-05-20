# Deep Modules

A **deep module** has a small interface and a large implementation — lots of behavior behind few methods and simple parameters.

A **shallow module** has a large interface and thin implementation — the caller does most of the work.

```
Deep:                          Shallow:
┌──────────────┐               ┌────────────────────────────┐
│  Small API   │               │       Large API            │
├──────────────┤               ├────────────────────────────┤
│              │               │  Thin pass-through         │
│  Complex     │               └────────────────────────────┘
│  behavior    │
│  hidden      │
│              │
└──────────────┘
```

## When Designing Interfaces, Ask

1. Can I reduce the number of methods?
2. Can I simplify the parameters?
3. Can I hide more complexity inside?
4. Apply the **deletion test**: if you deleted this module, would complexity vanish (shallow) or reappear across callers (deep)?

## Bowties Examples

**Deep store** — `configChanges.svelte.ts`:
- Interface: `visibleValue(key)`, `overrideValue(key, value)`, `resetField(key)`
- Implementation: multi-layer value resolution (draft → offlinePending → baseline), change tracking, dirty detection
- Why it's deep: callers get correct value without knowing the resolution chain

**Deep orchestrator** — `syncSession.ts`:
- Interface: `startSync()`, `cancelSync()`
- Implementation: multi-step workflow with node enumeration, diffing, conflict detection, write sequencing
- Why it's deep: caller triggers one action, orchestrator handles the entire lifecycle

**Shallow pattern to avoid** — a wrapper function that accepts the same parameters as its delegate and adds no behavior:
```typescript
// Shallow — just passes through
function getNodeDisplayName(nodeId: string): string {
  return nodeDisplayNameUtil.format(nodeId);
}
```

The deletion test shows this is shallow: deleting it just moves the `format()` call to each caller, with no complexity increase.
