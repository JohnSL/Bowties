# When to Mock

Mock at **system boundaries** only — where your code meets something external.

## Mock These (System Boundaries)

- **Tauri IPC** — mock `invoke()` in frontend tests to simulate backend responses
- **Network/transport** — mock TCP connections in backend and lcc-rs tests
- **Filesystem** — mock layout file reads/writes in backend tests
- **Time** — mock timers for timeout and retry testing

## Don't Mock These (Your Own Code)

- Your own stores, orchestrators, or utils
- Internal collaborators within a module
- Anything you control and can run in tests

If you're tempted to mock your own module, the architecture is wrong. The seam is in the wrong place. Consider:
- Can the module accept the dependency as a parameter instead?
- Can you test through the public interface of a higher-level module?
- Is the module too tightly coupled to its collaborators?

## Bowties Mocking Patterns

### Frontend: Mock Tauri Invoke

```typescript
// Mock the IPC boundary, not internal stores
vi.mock('$lib/api/tauri', () => ({
  invoke: vi.fn()
}));

// Set up specific command responses
const mockInvoke = vi.mocked(invoke);
mockInvoke.mockImplementation((cmd, args) => {
  if (cmd === 'get_node_info') return Promise.resolve(mockNodeInfo);
  if (cmd === 'read_config') return Promise.resolve(mockConfigValue);
});
```

### Backend: Mock Transport

```rust
// Mock the transport boundary, not internal domain modules
struct MockTransport {
    responses: VecDeque<Frame>,
}

impl Transport for MockTransport {
    fn send(&mut self, frame: Frame) -> Result<()> { Ok(()) }
    fn recv(&mut self) -> Result<Frame> {
        self.responses.pop_front().ok_or(Error::Timeout)
    }
}
```

### lcc-rs: Use In-Memory Adapters

```rust
// Provide an in-memory adapter at the transport seam
let (tx, rx) = channel();
let transport = ChannelTransport::new(tx, rx);
// Test protocol behavior through the real protocol module interface
```

## SDK-Style Interfaces Over Generic Fetchers

Create specific functions per operation — not one generic function with conditional logic:

```typescript
// Good — each function independently mockable
export const api = {
  fetchCdi: (nodeId: string) => invoke('fetch_cdi', { nodeId }),
  readConfig: (nodeId: string, address: number, size: number) =>
    invoke('read_config', { nodeId, address, size }),
  writeConfig: (nodeId: string, address: number, data: Uint8Array) =>
    invoke('write_config', { nodeId, address, data }),
};

// Avoid — mocking requires conditional logic
export const api = {
  call: (command: string, args: any) => invoke(command, args),
};
```
