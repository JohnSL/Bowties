# Interface Design for Testability

Good interfaces make testing natural. Design for these properties:

## 1. Accept Dependencies, Don't Create Them

```typescript
// Testable — dependency injected
function startDiscovery(transport: Transport, nodeStore: NodeStore) {
  // ...
}

// Hard to test — creates its own dependency
function startDiscovery() {
  const transport = new TcpTransport(getConfig());
  // ...
}
```

In Bowties, orchestrators should accept stores and API functions as parameters or import them from well-known modules — not create transport connections internally.

## 2. Return Results, Don't Produce Side Effects

```typescript
// Testable — returns a result
function resolveDisplayName(snip: SnipData, nodeId: string): string {
  // fallback chain: user_name → manufacturer+model → model → nodeId
}

// Hard to test — mutates external state
function updateDisplayName(nodeId: string): void {
  const name = /* resolve */;
  nodeStore.setName(nodeId, name);  // side effect
}
```

In Bowties, utils should be pure functions. Stores should expose deterministic transitions. Orchestrators own side effects but sequence them through store and API interfaces.

## 3. Small Surface Area

Fewer methods = fewer tests needed. Fewer parameters = simpler test setup.

```typescript
// Good — one method does the work
const configChanges = {
  visibleValue(key: string): ConfigValue { /* complex resolution */ }
};

// Avoid — caller must coordinate
const configChanges = {
  getDraft(key: string): ConfigValue | null { /* ... */ },
  getOfflinePending(key: string): ConfigValue | null { /* ... */ },
  getBaseline(key: string): ConfigValue | null { /* ... */ },
  // Caller must implement the fallback chain themselves
};
```

## Bowties Patterns

**Tauri commands** should have focused interfaces:
- Each command does one thing (read config, write config, fetch CDI — not a generic "do operation")
- Parameters are typed (not generic JSON objects)
- Returns structured results (not raw strings the caller must parse)

**Store interfaces** should expose behavior, not internal structure:
- `visibleValue(key)` — not `getDraft(key) ?? getOffline(key) ?? getBaseline(key)`
- `enrichedBowties()` — not `rawBowties()` + separate enrichment in every consumer

**Orchestrator interfaces** should be triggers, not coordination APIs:
- `startSync()` — not `enumerateNodes()` then `diffValues()` then `writeChanges()`
