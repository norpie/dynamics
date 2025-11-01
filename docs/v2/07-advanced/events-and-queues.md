# Events & Queue System

**Prerequisites:** [App and Context](../01-fundamentals/app-and-context.md)

---

## Consolidated Documentation

V2 provides **two parallel communication systems** for inter-app communication. These systems have been split into focused documents:

### 1. Event Broadcast System

See [Event Broadcast](events.md) for:
- Fire-and-forget pub/sub notifications
- Multiple subscribers
- Best-effort delivery
- Persistent subscriptions for background apps
- Type-safe event publishing and polling
- Examples: theme changes, migration selection, file saves

**Use for:** State change notifications to multiple apps

### 2. Work Queue System

See [Work Queues](queues.md) for:
- Exactly-once delivery
- Single consumer (queue owner)
- Priority-based processing
- SQLite persistence and crash recovery
- Type-safe work item handling
- Examples: API operations, batch jobs, background tasks

**Use for:** Guaranteed delivery work items to specific apps

---

## Quick Decision Guide

| Scenario | System |
|----------|--------|
| Notify multiple apps of state change | [Events](events.md) |
| Send work to specific app with guaranteed delivery | [Queues](queues.md) |
| State change that apps need to know about | [Events](events.md) |
| Operation that must not be lost (API call, file write) | [Queues](queues.md) |

---

## See Also

- [Event Broadcast](events.md) - Pub/sub system
- [Work Queues](queues.md) - Task queue system
- [App and Context](../01-fundamentals/app-and-context.md) - Context API reference
- [Background Work](../03-state-management/background-work.md) - Async task patterns

---

**Next:** Read [Event Broadcast](events.md) or [Work Queues](queues.md) for detailed documentation.
