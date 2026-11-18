# Good and Bad Tests

## Good Tests

**Integration-style**: test through real interfaces, not mocks of internal parts.

```typescript
// GOOD: Tests observable behavior through the store interface
test('visible value shows draft over baseline', () => {
  const store = createConfigChanges();
  store.setBaseline('05.01.01.00.00.00:253:42', 7);
  store.setDraft('05.01.01.00.00.00:253:42', 12);

  expect(store.visibleValue('05.01.01.00.00.00:253:42')).toBe(12);
});
```

Characteristics:
- Tests behavior users/callers care about
- Uses public API only
- Survives internal refactors
- Describes WHAT, not HOW
- One logical assertion per test

```typescript
// GOOD: Tests orchestrator workflow outcome
test('sync session writes changed values to node', async () => {
  mockInvoke.mockResolvedValueOnce(/* node info */);
  mockInvoke.mockResolvedValueOnce(/* baseline values */);

  const result = await startSync(nodeId, changes);

  expect(result.status).toBe('complete');
  expect(result.writtenCount).toBe(3);
});
```

```rust
// GOOD: Tests protocol behavior through public interface
#[test]
fn cdi_parser_extracts_groups_and_fields() {
    let xml = include_str!("fixtures/sample.cdi.xml");
    let tree = parse_cdi(xml).unwrap();

    assert_eq!(tree.segments.len(), 2);
    assert_eq!(tree.segments[0].groups.len(), 4);
    assert_eq!(tree.segments[0].groups[0].name, "Identification");
}
```

## Bad Tests

**Implementation-detail tests**: coupled to internal structure.

```typescript
// BAD: Tests implementation details — mocks internal collaborator
test('sync calls nodeRegistry.getNodes', async () => {
  const mockRegistry = vi.fn();
  await startSync(mockRegistry);
  expect(mockRegistry).toHaveBeenCalledWith('connected');
});
```

Red flags:
- Mocking internal collaborators (your own stores, utils, or orchestrators)
- Testing private methods or internal state
- Asserting on call counts or call order of internal functions
- Test breaks when refactoring without behavior change
- Test name describes HOW not WHAT
- Verifying through external means instead of through the interface

```typescript
// BAD: Bypasses store interface to verify
test('setDraft updates internal map', () => {
  const store = createConfigChanges();
  store.setDraft('key', 42);
  // Reaches into internals
  expect(store._drafts.get('key')).toBe(42);
});

// GOOD: Verifies through interface
test('draft value is visible after setting', () => {
  const store = createConfigChanges();
  store.setDraft('key', 42);
  expect(store.visibleValue('key')).toBe(42);
});
```

```rust
// BAD: Tests parsing internals
#[test]
fn parser_calls_handle_start_element() {
    let mut parser = CdiParser::new();
    parser.handle_start_element("group", &attrs);
    assert_eq!(parser.stack.len(), 1); // internal state
}

// GOOD: Tests parsing outcome
#[test]
fn parser_produces_group_with_name() {
    let tree = parse_cdi("<group><name>Outputs</name></group>").unwrap();
    assert_eq!(tree.groups[0].name, "Outputs");
}
```
