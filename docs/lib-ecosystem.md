# The `lib*` ecosystem

## Rule

Cross-cutting concerns are solved **once** in a shared `lib*` crate and reused by every repo,
not re-implemented. Before writing infrastructure code (errors, logging, config, paths,
signals, subprocesses), check whether a `lib*` already covers it. They're consumed as git deps
([rust-conventions.md](rust-conventions.md)).

## Catalogue

| Crate | Purpose | Key API |
|---|---|---|
| **liberror** | Serializable error wrapper preserving the source chain | `AnyError { $type, context }`, `From<E: Error>` |
| **liblog** | tracing + OpenTelemetry subscriber stack | `builder().endpoint(..).sample_rate(..).init() -> guard`; `force_cleanup(guard)` |
| **libsignal** | Cross-platform graceful shutdown (Unix/Windows signals) | `wait_for_signal()`, `cancel_after_signal(token)`, `*_or(token, fut)` |
| **libconfig** | TOML + env config with mtime tracking | `load<T>()`, `store_checked()`, `config!{}` macro |
| **libpath** | Platform app dirs (config/logs/cache/data) | `dirs::config_path()`, `logs_root()`, … (keyed by product name) |
| **libproduct** | Reverse-domain product descriptor + macro | `product_name!("dev.thmsn.x")`, `ProductName { base, ext, version, build }` |
| **libbuildinfo** | Build metadata (git/OS/package) embedded at build time | `emit()` (build.rs), `load_build_info!(optional)` |
| **libcmd** | Async subprocess exec with streaming + cancellation | `run(cmd, args, token) -> CommandMonitor` |
| **libwhich** | Binary discovery in `PATH` | `which(names)`, `is_valid_executable()` |
| **libring** | Fixed-size ring buffer for bounded history | `RingBuffer::new(cap)`, `.push()`, `.to_vec()` (opt. serde) |
| **libfs** | Tokio async filesystem with optional tracing | `PathResult<T>` over `AnyError` |

`libpath` + `libproduct` always travel together (paths are keyed by the product name).
`libbuildinfo` backs `libproduct`'s descriptor and `liblog`'s `service.build_info`.

## Graceful shutdown (`libsignal`)

The standard choreography — turn a signal into a `CancellationToken`, then race it:

```rust
let token = CancellationToken::new();
libsignal::cancel_after_signal(token.clone());     // spawns a waiter that cancels on SIGTERM/Ctrl-C

loop {
    tokio::select! {
        () = token.cancelled() => break,
        _  = do_work() => {}
    }
}
```

`cancel_after_signal_or(token, fut)` adds a second trigger (timeout, health failure). HTTP
services pass the same idea to poem's `run_with_graceful_shutdown`
([service-architecture.md](service-architecture.md)).

## Adding a new `lib*`

1. Edition 2024, workspace lints, `liberror`-based serializable errors
   ([error-handling.md](error-handling.md)).
2. One clear purpose; feature-gate optional surface; README documents the public API; examples
   are the integration tests ([archetypes/library.md](archetypes/library.md)).
3. Distribute as a git dep (HTTPS public / SSH private).

## Checklist

- [ ] Reached for an existing `lib*` before writing errors/logging/config/paths/signals/subprocess code.
- [ ] `libsignal` used for graceful shutdown; `liberror` for serializable errors; `liblog` for telemetry.
- [ ] `libpath` + `libproduct` used together for app dirs + product identity.
- [ ] A new shared concern becomes a new `lib*`, not a copy-paste.
