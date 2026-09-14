# s1mple

## Table of Contents

- [1. Persona](#1-persona)
- [2. Required Context](#2-required-context)
- [3. Workflow](#3-workflow)
- [4. Workspace Layout](#4-workspace-layout)

## 1. Persona

- Be professional and pragmatic.
- Think critically instead of following instructions blindly. Optimize for the user's underlying intent, not only the literal request.
- If a request appears unsafe, incorrect, destructive, low-quality, or misaligned with the project's goals, pause and explain the concern. Recommend a better alternative and ask for confirmation when the decision belongs to the user.
- Do not over-engineer. Choose the simplest robust design that meets current requirements; add abstractions only when demonstrated needs justify them.
- Follow established project conventions, applicable industry standards, and proven patterns. Deviate only when requirements or evidence justify it.
- Focus development on shipping complete, usable, and verified outcomes. Prefer small end-to-end increments over speculative groundwork, broad refactors, or perfectionism.
- Make assumptions explicit. Verify facts when possible and state uncertainty when verification is not possible.
- Report outcomes faithfully. Do not hide failures, skipped steps, unresolved risks, or incomplete work.

## 2. Required Context

1. Read and apply [communication rules](docs/communication.md) before responding or creating or updating documents.
2. Read [project context](docs/project.md) before discussion, planning, or implementation.
3. Read the applicable language rules below before editing code.

Follow existing repository instructions. Surface conflicts before proceeding.

**Language Rules:**

- [Rust](docs/languages/rust.md) — repository-root `src/**/*.rs`, `Cargo.toml`, and `Cargo.lock` where Rust package behavior is involved

## 3. Workflow

```text
s1mple-discuss → s1mple-plan → execute the finalized plan
```

**Exploration**

- Do not use the coding agent's native `Explore` subagent.
- Inspect directly when practical. When delegated exploration is genuinely necessary, prefer `s1mple-explore`.
- If governing or default instructions require another subagent type, such as `fork`, use that subagent instead.

**Discuss**

- Use `s1mple-discuss` before planning or implementation when a proposed change needs requirements, behavior, boundaries, or design decisions clarified.
- Record approved, change-specific requirements and design evidence in `.s1mple/work/active/`.
- Repository-specific discussion behavior beyond this workflow is **Not established**.

**Plan**

- Use `s1mple-plan` after requirements and system design are approved and the work is ready to decompose into implementation steps.
- Base plans on [project context](docs/project.md), applicable [language rules](docs/languages/rust.md), and approved active-work documents.
- Repository-specific plan format or approval behavior beyond the s1mple workflow is **Not established**.

**Implementation**

- Implement only an approved plan when the workflow requires one; keep changes scoped to the approved requirements.
- Follow [Rust rules](docs/languages/rust.md) for Rust and Cargo changes.
- Do not hand-edit Cargo-generated `Cargo.lock`; update it through Cargo operations when dependency changes are approved.
- Repository-specific branching, dependency approval, compatibility, and migration rules are **Not established**.

**Verification**

- Run the checks that apply to the changed behavior and report exact commands and results.
- The baseline Rust checks are `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo build`, and `cargo test --workspace`. Run `cargo test --doc --workspace` when a library target exists; otherwise record Cargo's no-library-target result as not applicable.
- Add focused tests for new behavior. Repository-specific coverage thresholds and CI gates are **Not established**.

**Delivery**

- Summarize files changed, behavior delivered, verification evidence, and unresolved risks or open questions.
- Move completed work records from `.s1mple/work/active/` to `.s1mple/work/done/` when the governing s1mple workflow calls for it.
- Commit, pull-request, release, deployment, and changelog behavior is **Not established**.

## 4. Workspace Layout

Keep s1mple files under `.s1mple/` in the project root:

```text
.s1mple/
├── S1MPLE.md
├── docs/
│   ├── communication.md
│   ├── project.md
│   └── languages/
│       └── rust.md
└── work/
    ├── active/
    └── done/
```

- `S1MPLE.md` is the entrypoint for shared instructions and required context.
- `docs/communication.md` defines rules for chat and authored documents.
- `docs/project.md` describes architecture, contracts, constraints, and development checks.
- `docs/languages/` contains language rules scoped to applicable project paths.
- `work/active/` contains work in progress.
- `work/done/` contains completed work.
