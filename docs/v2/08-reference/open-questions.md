# Open Questions / TODO

**Status:** All major design items completed

This document tracks open design questions and unresolved items for the V2 TUI framework.

---

## Current Status

As of the latest design iteration, all major architectural decisions have been finalized:

- ✅ App trait and Context API
- ✅ Layer system
- ✅ Focus management
- ✅ Mouse support
- ✅ Component patterns
- ✅ Resource pattern with progress tracking
- ✅ Error recovery strategies
- ✅ Events & queue system
- ✅ Theme system
- ✅ Keybind system
- ✅ App launcher
- ✅ Help system
- ✅ Background apps
- ✅ Animation system

---

## Future Considerations

Items that may be revisited after initial implementation:

### Performance Optimizations
- Virtual scrolling for very large lists (10,000+ items)
- View diffing to minimize re-renders
- Async rendering pipeline

### Developer Experience
- Hot reload for rapid iteration
- Debug overlay showing focus state, layer boundaries
- Visual component inspector

### Advanced Features
- Time-travel debugging
- Undo/redo framework
- Clipboard integration
- Drag & drop between apps

---

## Notes

These items are **nice-to-have** features that don't block the V2 implementation. They should be considered based on real usage patterns after the core framework is stable.

---

**See Also:**
- [Non-Goals](../00-overview.md#non-goals) - Explicitly excluded features
- [Next Steps](../00-overview.md#next-steps) - Implementation roadmap
