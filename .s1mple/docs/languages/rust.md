# Rust

## Applies To

- **Paths:** Repository-root `src/**/*.rs`, `Cargo.toml`, and `Cargo.lock` where Rust package behavior is involved.
- **Component Differences:** None. The repository currently contains one binary crate.

## Toolchain and Configuration

| Tool | Version or configuration source | Purpose |
|---|---|---|
| Rust compiler | Edition 2024 from [repository-root `Cargo.toml`](../../../Cargo.toml); installed `rustc 1.96.0` observed on 2026-09-14 | Compile Rust source and enforce edition semantics |
| Cargo | Package configuration in [repository-root `Cargo.toml`](../../../Cargo.toml); installed `cargo 1.96.0` observed on 2026-09-14 | Resolve dependencies, build, test, lint, and run the crate |
| rustfmt | Default formatting; no repository `rustfmt.toml` was found | Format Rust source consistently |
| Clippy | No repository lint configuration was found | Detect correctness, style, complexity, and performance issues |

## Coding Conventions

Rust code must prioritize correctness, standards compliance, readability, and maintainability. Encode invariants in types, handle failures explicitly, validate external input at boundaries, and add focused tests for meaningful behavior.

| Convention | Basis | Applies to | Source |
|---|---|---|---|
| Use Rust edition 2024 | Tool-enforced | All Rust source | [repository-root `Cargo.toml`](../../../Cargo.toml) |
| Use default rustfmt formatting; do not hand-format code that rustfmt owns | Documented project guidance | Repository-root `src/**/*.rs` | This document |
| Use `snake_case` for crates, modules, functions, methods, and variables | Documented project guidance | Repository-root `src/**/*.rs` | This document |
| Use `UpperCamelCase` for types, traits, and enum variants | Documented project guidance | Repository-root `src/**/*.rs` | This document |
| Use `SCREAMING_SNAKE_CASE` for constants and statics | Documented project guidance | Repository-root `src/**/*.rs` | This document |
| Keep `main.rs` focused on entrypoint wiring as the program grows | Documented project guidance | Repository-root `src/main.rs` | This document |
| Prefer the standard library before adding a dependency | Documented project guidance | `Cargo.toml` and Rust source | This document |
| Treat `Cargo.lock` as Cargo-generated and do not edit it manually | Tool-enforced by generated-file declaration | Repository-root `Cargo.lock` | [repository-root `Cargo.lock`](../../../Cargo.lock) |

### Formatting and Documentation

- Run `cargo fmt --check` before marking Rust changes complete.
- Keep rustfmt configuration minimal and stable. If a future style-edition difference matters, configure it explicitly so editors and CI agree.
- Use `//!` for crate or module documentation and `///` for public API items.
- Document public contract behavior, constraints, units, errors, panics, and safety requirements when they are not evident from the signature.
- Keep documentation examples minimal and doctestable. Use `no_run` or `ignore` only when execution is impractical.
- For private code, comment on non-obvious reasons, invariants, compatibility constraints, or safety assumptions—not mechanics visible from the code.

### Modules and Control Flow

- Organize modules around cohesive domain concepts rather than generic technical buckets.
- Keep `main.rs` and any future `lib.rs` focused on crate wiring and public surface area.
- Re-export items intentionally because re-exports become part of the API.
- Avoid wildcard imports outside tests and tightly scoped prelude-style modules.
- Prefer `match` when exhaustive variant handling matters.
- Prefer `if let` or `let ... else` when only one branch is interesting.
- Use early returns for validation and error paths to avoid deeply nested success paths.
- Avoid `_` in matches over public-contract enums when a new variant should force review.

### Abstraction and Ownership

- Prefer concrete types until multiple demonstrated callers need abstraction.
- Keep traits small and behavior-focused. Do not introduce a trait solely to mock a dependency; prefer testing observable behavior.
- Use generics only when callers benefit without making types or errors harder to understand.
- Use macros only when functions, traits, or derives cannot express the pattern clearly.
- Accept borrowed values such as `&str`, `&[T]`, and `&Path` when ownership is unnecessary.
- Take ownership when storing a value, spawning work, or transferring responsibility.
- Do not clone merely to satisfy the borrow checker before understanding the intended ownership model.
- Return iterators when lazy consumption is part of the contract; return collections when materialization is part of the result.

### Dependencies, Features, and Concurrency

- Add third-party crates only when they are maintained, appropriately licensed, and reduce total project complexity.
- Keep dependency features explicit; avoid broad default features when only a narrow set is needed.
- Keep feature flags additive. Enabling one must not silently disable unrelated behavior.
- Do not claim or change a minimum supported Rust version unless a project requirement establishes one.
- Use concurrency only when required by the workload or when it clearly simplifies ownership of work.
- Prefer message passing or owned task state over shared mutable state.
- Use `Arc` for shared ownership and add `Mutex` or `RwLock` only when state must mutate across owners.
- Never hold a blocking lock across an `.await` point.
- Make cancellation and shutdown explicit for long-running tasks.

## Types and Public Interfaces

- No public library interface currently exists.
- If public APIs are introduced, keep fields private unless direct access is the intended contract and use constructors or methods to preserve invariants.
- Use standard conversion and traversal traits (`From`, `TryFrom`, `AsRef`, `AsMut`, `Borrow`, `Iterator`, and `IntoIterator`) instead of ad-hoc method names when their semantics fit.
- Use `iter`, `iter_mut`, and `into_iter` for collection traversal methods.
- Derive common traits such as `Debug`, `Clone`, `Copy`, `Eq`, `PartialEq`, `Ord`, `PartialOrd`, `Hash`, and `Default` only when semantically correct.
- Implement `Display` for user-facing formatting and keep `Debug` useful for diagnostics.
- Use builders when construction has many optional or order-independent fields.
- Treat every public field, function, type, trait, implementation, and re-export as a compatibility commitment that requires deliberate review.

## Error Handling

- Return `Result<T, E>` for recoverable failures.
- Reserve `panic!`, `expect`, and `unwrap` for violated invariants, test setup failures, or unrecoverable startup/configuration failures with clear context.
- Library code should expose meaningful structured errors that implement `std::error::Error`.
- Application code should add context at boundaries where the attempted operation is known.
- Do not erase structured errors into strings before callers can inspect them.
- Validate and parse untrusted input at system boundaries before passing typed values inward.
- Do not log secrets, tokens, credentials, or raw sensitive payloads.
- Avoid shell commands built from interpolated user input; pass arguments as structured process arguments.
- Current repository-specific error types and logging conventions are **Not established** because the program has no fallible operations ([repository-root `src/main.rs`](../../../src/main.rs)).

## Unsafe Code

- Do not introduce `unsafe` unless no safe Rust alternative meets an established requirement.
- Keep unsafe blocks minimal and isolate them behind safe APIs.
- Document each unsafe block with the invariant that makes it sound.
- Add tests that exercise the safe API around unsafe behavior.

## Testing and Verification

- Add focused unit tests beside internal logic.
- Add integration tests under repository-root `tests/` when behavior crosses the binary or crate boundary.
- Keep public documentation examples doctestable when a library target or public API is introduced.
- Test success paths, expected failures, and important boundary conditions. Do not add tests that only mirror implementation details.
- The repository currently contains no explicit tests, custom test runner, rustfmt configuration, Clippy configuration, library target, or CI configuration.

| Check | Working directory | Command or method | Evidence |
|---|---|---|---|
| Format | Repository root | `cargo fmt --check` | Passed on 2026-09-14 |
| Lint | Repository root | `cargo clippy --all-targets --all-features -- -D warnings` | Passed on 2026-09-14; no issues found |
| Build | Repository root | `cargo build` | Passed on 2026-09-14 |
| Tests | Repository root | `cargo test --workspace` | Passed on 2026-09-14; 0 tests found |
| Documentation tests | Repository root | `cargo test --doc --workspace` | Not applicable on 2026-09-14; Cargo reported no library targets |

When a command does not apply, record the exact reason rather than silently skipping it.

## Examples

Current entrypoint behavior at [repository-root `src/main.rs`](../../../src/main.rs):

```rust
fn main() {
    println!("Hello, world!");
}
```

This example documents current behavior only; it does not establish application architecture.

## Sources and Open Questions

- **Sources:** This self-contained language guide, [repository-root `Cargo.toml`](../../../Cargo.toml), [repository-root `Cargo.lock`](../../../Cargo.lock), and [repository-root `src/main.rs`](../../../src/main.rs).
- **Open Questions:** Minimum supported Rust version, application error type strategy, test coverage expectations, lint customization, CI requirements, release checks, and any exception to these baseline rules are not established.
