## Summary

-

## Validation

- [ ] Ran `cd app && npm run test:refactor-gate` when touching offline/sync/discovery UI behavior or lifecycle orchestration
- [ ] Ran any additional targeted tests needed for this change

## Architecture Checklist

- [ ] View components remain focused on rendering state and emitting intent events, or this PR explains why business branching could not be extracted
- [ ] Store/orchestrator changes keep lifecycle transitions covered by focused tests or route-level workflow tests