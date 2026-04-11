# Performance Optimization Rationale

The following performance bottlenecks were identified in the device state persistence logic:

1. **Mutex Contention in `dirty_flag`**: The `DeviceStateStore` uses `Arc<Mutex<bool>>` to track if the state is dirty. If the lock is contended, it spawns a new Tokio task just to set the boolean. Replacing this with `Arc<AtomicBool>` eliminates the need for locks and task spawning, making `mark_dirty()` a very cheap operation.

2. **Synchronous JSON Serialization**: In `save_async`, `serde_json::to_string_pretty` is called on the async thread before entering `spawn_blocking`. For large device states, this CPU-intensive operation can block the async executor, increasing latency for other tasks. Moving serialization into the `spawn_blocking` block ensures the async executor remains responsive.

3. **Repeated Lock Acquisition in Loops**: The `save_final_device_states` function in `src/main.rs` iterates over devices and calls `update_keyboard` for each one. Each call acquires and releases a write lock on the internal state map. Implementing a batch update method allows all states to be updated with a single lock acquisition, reducing overhead.

4. **Lack of Awaited State Flush at Shutdown**: `save_final_device_states` currently "fires and forgets" updates to the state store during shutdown. Since the background write task might be aborted when the store is dropped, the final device states are often lost. Adding an awaited `save()` call ensures persistence while remaining within the async paradigm.
