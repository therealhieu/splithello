# W260914RUZU splithello-start Execution Goal

## Status

- **State:** `Complete`
- **Current Position:** `Complete — all groups, overall remediation, verification evidence, and project documentation finalized`
- **Last Updated:** `2026-09-14`

## Sources

- s1mple instructions: `.s1mple/S1MPLE.md`
- Project document: `.s1mple/docs/project.md`
- Plan index: `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-index.md`
- System design: `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-system-design.md`
- Requirements: `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-requirements.md`
- Plan context: `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-context.md`
- Roadmap binding: Not applicable — standalone work
- Current parent sources: Not applicable — standalone work
- Project standards:
  - `.s1mple/docs/communication.md`
  - `.s1mple/docs/languages/rust.md`
- Plan parts:
  1. `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-1-foundation-contracts.md`
  2. `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-2-protocol-components.md`
  3. `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-3-proxy-runtime.md`
  4. `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-4-integration-documentation.md`

## Execution Contract

- **Goal:** Replace the placeholder with a clean-room, safe-Rust, unprivileged loopback CONNECT/PAC application that starts through `splithello start --config <path>`, uses authenticated target-only DoH and bounded byte-preserving TLS ClientHello record fragmentation for configured Reddit and Medium domains, preserves normal unchanged direct IPv4/IPv6 relay for unselected hosts, and ships deterministic verification plus accurate operating documentation.
- **Scope:** Implement all four finalized plan parts, eight groups, nineteen tasks, `S1`–`S17`, `V1`–`V21`, the nine overall checks, required group reports, overall-verification report, remediation when required, and verified project-document finalization.
- **Non-goals:** Transparent interception; root, firewall, NFQUEUE, eBPF, raw sockets, TUN/TAP, fake packets, TTL or TCP-sequence manipulation; VPN or anonymity; client-IP hiding; selected-target IPv6 mitigation; QUIC/HTTP3; automatic system/browser setting mutation; public-network acceptance gates; copied source or distinctive structure from the unlicensed reference project; push, release, deployment, or history rewriting.
- **Execution Order:** Part 1 foundation contracts → Part 2 protocol components → Part 3 proxy runtime → Part 4 integration/documentation → overall verification → project documentation finalization.
- **Workflow Behavior:** Implement only this finalized plan; keep every change task-scoped; use focused TDD where planned; add maintained licensed dependencies only when they reduce complexity; never hand-edit `Cargo.lock`; run and report exact checks; commit each task before the next; persist and separately commit every implementation and verification report; summarize behavior, files, evidence, limitations, and unresolved risks faithfully.
- **Binding Constraints:** One edition-2024 binary; crate-private cohesive modules; no `unsafe`; strict validation before side effects; loopback-only listeners; no invalid-certificate acceptance; `dns.ca_certificate` may only add PEM trust roots; selected target resolution uses authenticated DoH A/IPv4 only and never system DNS; direct unselected resolution preserves normal OS IPv4/IPv6 candidates; origin TLS remains end-to-end; bounded headers, buffers, DNS bodies/cache, timeouts, connection counts, and shutdown; no traffic payloads or secrets in logs; deterministic default tests require neither Internet nor root; clean-room implementation.
- **Assigned parent coverage:** Not applicable — standalone work
- **Cross-work gates:** None
- **Parent synchronization authorization:** Not applicable — standalone work
- **Reports Directory:** `.s1mple/work/active/w260914ruzu-splithello-start/reports/`
- **Implementation Report Naming:** `p<part-number>-g<group-number>-implementation-report.md`
- **Verification Report Naming:** `p<part-number>-g<group-number>-verification-report.md`
- **Reports:** Each group has exactly one implementation report and one verification report.
- **Overall Verification Prompt:** `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/overall-verification-prompt.md`
- **Overall Verification Report Template:** `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/overall-verification-report-template.md`
- **Overall Verification Report:** `.s1mple/work/active/w260914ruzu-splithello-start/reports/overall-verification-report.md`
- **Overall Remediation Prompt:** `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/overall-remediation-prompt.md`
- **Remediation:** Append group and overall remediation evidence to the responsible implementation report. Never create a remediation report.

## Execution Rules

- Start only when the user commands execution. Set this goal to `In Progress` before implementation.
- Self-drive from start to finish after that command. Choose the next dependency-ready action, investigate failures, use safe in-scope alternatives, verify fixes, and continue without further questions or approval requests.
- Stop only when no safe in-scope work can proceed. Record evidence and mark the goal `Blocked`; do not ask the user to decide routine implementation details.
- Do not bypass permissions, weaken tests, skip checks, change approved requirements/design/scope, or use a peer to perform a denied action.
- Execute the planned task and report commits. Do not push, amend, rebase, rewrite history, or discard unrelated work.
- Inspect staged changes before every commit. Commit only task-owned or report-owned files; preserve unrelated staged and unstaged changes.
- A task commit is an implementation checkpoint, not group acceptance. A report commit is an evidence checkpoint, not group acceptance.
- The group implementer commits task changes and any remediation fixes. The main orchestrator validates evidence and commits only the final `.s1mple/docs/project.md` update.
- Follow `.s1mple/docs/languages/rust.md`: explicit recoverable errors, validated boundaries, safe Rust, narrow APIs, default formatting, focused tests, and standard checks.
- Follow `.s1mple/docs/communication.md` for every report and user-visible result.

### Subagent Recovery

- Resume an old required subagent when possible.
- If it cannot be resumed, spawn a fresh subagent of the same type and preserve the original role restrictions.
- Give the replacement the original assignment, this goal, governing source paths, relevant plan part/group, committed outputs and reports, completed checks and SHAs, remaining work, constraints, and authorization boundaries.
- A replacement must not repeat a substantive verification pass or other work prohibited from repetition. A verifier replacement may retry persistence only when the original substantive verification already completed.
- If required handoff evidence is missing, mark the affected stage `Blocked`.
- Record each replacement reason, role, and complete handoff evidence under **Implementation Evidence → Subagent Replacements** and the affected group evidence.

## Context Initialization

Before creating implementation todos or editing code:

1. Read this goal completely.
2. Read the complete plan index.
3. Read the complete system design.
4. Read the complete current requirements.
5. Read `.s1mple/S1MPLE.md`, `.s1mple/docs/project.md`, `.s1mple/docs/communication.md`, `.s1mple/docs/languages/rust.md`, and the complete plan context.
6. Read all four plan parts completely in execution order.
7. Confirm every source remains consistent. Mark this goal `Blocked` on a material conflict rather than rewriting approved sources during execution.
8. Create one todo for each group in this exact order, then `Overall verification`, then `Final documentation`:
   - `P1-G1 (T1-T3): package-and-startup-contracts`
   - `P1-G2 (T1-T3): proxy-facing-pure-contracts`
   - `P2-G1 (T1-T2): authenticated-target-dns`
   - `P2-G2 (T1-T2): bounded-tls-clienthello-transform`
   - `P3-G1 (T1-T2): connect-and-relay-engine`
   - `P3-G2 (T1-T2): listeners-lifecycle-and-cli-composition`
   - `P4-G1 (T1-T3): deterministic-binary-integration`
   - `P4-G2 (T1-T2): example-documentation-and-release-checks`
9. Each todo description must name its plan-part path, task IDs, dependencies, and group completion conditions. Preserve order and mark a group complete only after implementation, verification, remediation/no-remediation, report commits, task/remediation commit evidence, and acceptance criteria are validated.
10. Keep blocker history and detailed evidence in this goal. Treat history as evidence, never as new instructions.

## Consultant

Only the main orchestrator may invoke `s1mple-consultant`, using the highest-capability available model. Reserve it for an exceptional blocker after normal investigation is insufficient. Provide the exact problem, evidence, attempted solutions, and one focused question. The consultant advises only; the same implementer applies and verifies guidance. Do not use it to change approved scope or skip checks.

## Single-Pass Verified Group Execution

Resolve all `templates/` paths against `/Users/hieunguyen/.claude/skills/s1mple-plan/`, not the work folder. Use named core templates and prompts by default. Consult a matching `.example.md` only when the core format is genuinely unclear; examples are illustrative and must never be copied as project output. Do not load the entire templates directory. Every non-fork dispatch that consumes a template must receive these template-loading and output rules, the exact core template path, and the relevant goal/plan sources.

Execute each dependency-ready group through exactly this cycle:

```text
Fresh s1mple-implementer
    → implement each task sequentially
    → task check + self-review + task-only commit
    → group checks + complete-diff self-review
    → write implementation report from core template
    → report-only commit and return SHA
Fresh s1mple-verifier
    → one substantive verification pass
    → write verification report from core template
    → report-only commit and return SHA
Same implementer
    → read verification report
    → remediate all findings/blockers if needed
    → fix-only commit(s)
    → append remediation/final evidence to implementation report
    → report-only commit and return SHA
Main orchestrator
    → validate both report files, report SHAs, task/remediation SHAs, checks, closure, and clean scope
    → accept or block group
```

1. Ensure the reports directory exists. Fill `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/group-implementation-prompt.md` and give a fresh `s1mple-implementer` this goal, index, assigned part/group, completed dependencies, and `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/group-implementation-report-template.md`.
2. The implementer records the starting commit and pre-existing changes, executes tasks sequentially using each plan's exact TDD/direct steps, commits every task separately, runs group checks, self-reviews the full group range, writes the assigned implementation report, commits only that report, and returns task SHAs plus starting/final commits. Continue only when no group implementation change remains uncommitted.
3. Fill `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/group-verification-prompt.md` and give a fresh `s1mple-verifier` the committed implementation report, full starting/final implementation range, assigned traceability, and `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/group-verification-report-template.md`.
4. The verifier performs substantive verification exactly once, writes the assigned report for `Passed`, `Failed`, or `Blocked`, commits only that report, and returns its SHA. Finding IDs must use the active group prefix: `P1-G1-VER<number>`, `P1-G2-VER<number>`, `P2-G1-VER<number>`, `P2-G2-VER<number>`, `P3-G1-VER<number>`, `P3-G2-VER<number>`, `P4-G1-VER<number>`, or `P4-G2-VER<number>`.
5. Pass both reports to the same implementer. If the verification report has findings, blockers, or unverified required checks, fill `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/group-remediation-prompt.md`. Keep the verification report unchanged. Do not create a remediation report.
6. The implementer resolves every finding/check within scope, runs affected task/group checks, commits fixes separately, appends exact remediation and final evidence to the existing implementation report, and commits only the finalized report. When verification passed with no findings, record `Not required` and keep the initial implementation-report commit as final.
7. The orchestrator validates report existence and commits, task/remediation evidence, exact closure, acceptance criteria, required checks, and clean scoped status. Do not synthesize or rerun substantive verification. Mark a group complete only when all evidence is persisted and no required issue remains.

## Execution Flow

```text
Part 1: foundation-contracts
  P1-G1 package-and-startup-contracts
    → P1-G2 proxy-facing-pure-contracts
Part 2: protocol-components
  P2-G1 authenticated-target-dns
    → P2-G2 bounded-tls-clienthello-transform
Part 3: proxy-runtime
  P3-G1 connect-and-relay-engine
    → P3-G2 listeners-lifecycle-and-cli-composition
Part 4: integration-documentation
  P4-G1 deterministic-binary-integration
    → P4-G2 example-documentation-and-release-checks
→ Overall verification
→ Project documentation finalization
→ Complete
```

## Progress

### Part 1 — foundation-contracts

- **Plan:** `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-1-foundation-contracts.md`
- **Dependencies:** None
- **Status:** `Completed and accepted`

#### P1-G1 — package-and-startup-contracts

- [x] Context and dependencies confirmed.
- [x] Implementation turn completed.
- [x] `P1-G1-T1` — declare-the-minimal-approved-dependency-graph.
- [x] `P1-G1-T2` — implement-cli-and-strict-configuration.
- [x] `P1-G1-T3` — implement-canonical-domain-policy.
- [x] Every task has commit SHA(s) or justified no-change evidence.
- [x] Task-level and group-level checks passed.
- [x] Initial implementation report persisted and separately committed.
- [x] Verification report persisted and separately committed.
- [x] Report status checked and remediation completed or not required.
- [x] Final implementation report commit recorded.
- [x] Orchestrator validated closure, commit evidence, acceptance criteria, and no uncommitted group changes.

**Evidence:**

- Implementation: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p1-g1-implementation-report.md` — `Completed`
- Initial implementation report commit: `779c132d8de55060926988caf1cb08d3ff73da5b`
- Verification: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p1-g1-verification-report.md` — `Failed — P1-G1-VER1 requires exact V2 ConfigError assertions`
- Verification report commit: `9ff77b93bc2dc382f6aa3b88f4f21e242e1519a2`
- Remediation: `Completed — P1-G1-VER1 fixed with exact ConfigError matrix assertions`
- Final implementation report commit: `279760b5eff07c795929ac4195e97babb6f9a064`
- Remediation commits: `c672acdc2999bb15c9f25d8de9970089d6659d94`
- Closure evidence: `Accepted — finding checklist closed; focused config 5/5, workspace 10/10, domain 5/5, CLI 2/2, help, format, clippy, all-target check, dependency boundary, commit scope, and clean group status validated`

**Task Commit Evidence:**

| Task | SHA(s) or justified no-change evidence |
|---|---|
| `P1-G1-T1` | `8e54c59f2586652a39b571ab7c1d730a80f1af6e` |
| `P1-G1-T2` | `6b75b10c9d26b5e7d1a4bea72210968f2bcda6ba`, follow-up coverage `c8676aeb60337545a419fea9470251752b09cc2a` |
| `P1-G1-T3` | `9ab97bdac42ecef52c3a4d778a9fb77208b0dbf1` |

#### P1-G2 — proxy-facing-pure-contracts

- [x] Context and dependency `P1-G1` confirmed.
- [x] Implementation turn completed.
- [x] `P1-G2-T1` — implement-bounded-connect-parsing.
- [x] `P1-G2-T2` — implement-target-only-pac-rendering.
- [x] `P1-G2-T3` — define-secret-safe-diagnostic-contracts.
- [x] Every task has commit SHA(s) or justified no-change evidence.
- [x] Task-level and group-level checks passed.
- [x] Initial implementation report persisted and separately committed.
- [x] Verification report persisted and separately committed.
- [x] Report status checked and remediation completed or not required.
- [x] Final implementation report commit recorded.
- [x] Orchestrator validated closure, commit evidence, acceptance criteria, and no uncommitted group changes.

**Evidence:**

- Implementation: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p1-g2-implementation-report.md` — `Completed`
- Initial implementation report commit: `b4f74902cdae053da038f968018768ca14fc4601`
- Verification: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p1-g2-verification-report.md` — `Failed — P1-G2-VER1 requires exact V6 ProxyError/status assertions`
- Verification report commit: `3a579a3246982b6fcd45877e9f563142ed12b20c`
- Remediation: `Completed — P1-G2-VER1 fixed with exact V6 ProxyError/status assertions`
- Final implementation report commit: `4a63f09c1b41eeef11747cfde5dafcd0dc47bbf4`
- Remediation commits: `31f244962874055afa28a9324d490da4e02d62bc`
- Closure evidence: `Accepted — all nine named malformed authority/header fixtures assert exact errors/statuses; proxy 6/6, workspace 21/21, PAC 3/3, diagnostics 2/2, domain 6/6, format, clippy, build, all-target, scope, and isolation validated`

**Task Commit Evidence:**

| Task | SHA(s) or justified no-change evidence |
|---|---|
| `P1-G2-T1` | `db5652bbfeb2ccf3b6263c3a84ee698fae0f7005` |
| `P1-G2-T2` | `9e1792f60e6cf5dc9f287bc6401bfb740c4fc4ab` |
| `P1-G2-T3` | `e9d625b89275ad03de6d846b66ffc4b2660b8e7f` |

### Part 2 — protocol-components

- **Plan:** `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-2-protocol-components.md`
- **Dependencies:** Part 1 completed and accepted
- **Status:** `Completed and accepted`

#### P2-G1 — authenticated-target-dns

- [x] Context and Part 1 dependencies confirmed.
- [x] Implementation turn completed.
- [x] `P2-G1-T1` — implement-dns-message-validation-and-cache.
- [x] `P2-G1-T2` — implement-authenticated-doh-and-system-resolvers.
- [x] Every task has commit SHA(s) or justified no-change evidence.
- [x] Task-level and group-level checks passed.
- [x] Initial implementation report persisted and separately committed.
- [x] Verification report persisted and separately committed.
- [x] Report status checked and remediation completed or not required.
- [x] Final implementation report commit recorded.
- [x] Orchestrator validated closure, commit evidence, acceptance criteria, and no uncommitted group changes.

**Evidence:**

- Implementation: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p2-g1-implementation-report.md` — `Completed`
- Initial implementation report commit: `39dc4e994dc403c568c7d098f5bc0871ceba944c` (supersedes initial report commit `6f0f5ce43a099c5cb7522e71b4a66f96e3669d02` to correct commit evidence)
- Verification: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p2-g1-verification-report.md` — `Failed — P2-G1-VER1 requires complete V9 identity and streamed-size assertions`
- Verification report commit: `9b801134f9ca777f341a8474c392aabd6ac2a149`
- Remediation: `Completed — P2-G1-VER1 fixed with exact DNS identity mismatch and unknown-length streamed overflow assertions`
- Final implementation report commit: `353e62b719e56c50aaef36f03293b0d9110cd14a`
- Remediation commits: `1dacb05646c364851d078e30e8145acedf78cbb0`, `78b4261c3c1630996bf17dcab83462064e0df976`
- Closure evidence: `Accepted — DNS 10/10, workspace 31/31, clippy, format, build, all-target checks, exact finding checklist, complete diff, and isolation validated`

**Task Commit Evidence:**

| Task | SHA(s) or justified no-change evidence |
|---|---|
| `P2-G1-T1` | `3cc1d21a02c4d845165292db664d62845d601246`, follow-up `254174d166b78741fc5fcd4fc0ab8a62a5739ac2` |
| `P2-G1-T2` | `ddaa5dfec3b56130108b7767f68c3d0b29d48bfc`, follow-up `f80523e3c91786df745e51d0995b0a4e399ab096` |

#### P2-G2 — bounded-tls-clienthello-transform

- [x] Context and dependency `P2-G1` confirmed.
- [x] Implementation turn completed.
- [x] `P2-G2-T1` — parse-bounded-clienthello-across-records.
- [x] `P2-G2-T2` — serialize-byte-preserving-sni-record-split.
- [x] Every task has commit SHA(s) or justified no-change evidence.
- [x] Task-level and group-level checks passed.
- [x] Initial implementation report persisted and separately committed.
- [x] Verification report persisted and separately committed.
- [x] Report status checked and remediation completed or not required.
- [x] Final implementation report commit recorded.
- [x] Orchestrator validated closure, commit evidence, acceptance criteria, and no uncommitted group changes.

**Evidence:**

- Implementation: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p2-g2-implementation-report.md` — `Completed`
- Initial implementation report commit: `8d0102676f75ed8bad2c4d2ce2247a7e447b9c9c`
- Verification: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p2-g2-verification-report.md` — `Passed with no findings`
- Verification report commit: `8cbb95722ab202eede20de63d1ec159d020cc713`
- Remediation: `Not required`
- Final implementation report commit: `8d0102676f75ed8bad2c4d2ce2247a7e447b9c9c`
- Remediation commits: `None`
- Closure evidence: `Accepted — V11–V13/V21 and group checks passed; V15 component handoff verified and only planned Part 4 stream/process evidence remains deferred`

**Task Commit Evidence:**

| Task | SHA(s) or justified no-change evidence |
|---|---|
| `P2-G2-T1` | `a3ba8742e82260f6fccf511ff807d87769df916f`, follow-up `3480d3230732151eb2500fbcbcf2b801659c2fc0` |
| `P2-G2-T2` | `8e50bdec3f30d98822367d9755fb715fa333721a`, follow-up `5619f350cdc25544ad71df1a7db9d5847d06ad0f` |

### Part 3 — proxy-runtime

- **Plan:** `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-3-proxy-runtime.md`
- **Dependencies:** Parts 1 and 2 completed and accepted
- **Status:** `Completed and accepted`

#### P3-G1 — connect-and-relay-engine

- [x] Context and Parts 1–2 dependencies confirmed.
- [x] Implementation turn completed.
- [x] `P3-G1-T1` — implement-address-connection-and-direct-relay.
- [x] `P3-G1-T2` — implement-selected-target-tunnel.
- [x] Every task has commit SHA(s) or justified no-change evidence.
- [x] Task-level and group-level checks passed.
- [x] Initial implementation report persisted and separately committed.
- [x] Verification report persisted and separately committed.
- [x] Report status checked and remediation completed or not required.
- [x] Final implementation report commit recorded.
- [x] Orchestrator validated closure, commit evidence, acceptance criteria, and no uncommitted group changes.

**Evidence:**

- Implementation: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p3-g1-implementation-report.md` — `Completed`
- Initial implementation report commit: `6098dba95397f2559649be5dafa6becaaaa8c03c`
- Verification: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p3-g1-verification-report.md` — `Failed — P3-G1-VER1 through P3-G1-VER3`
- Verification report commit: `1ebd942cbf5883a7d2299151b3932d87cf123d32`
- Remediation: `Completed — bounded CONNECT reads, bounded/cancellable direct resolution, and concurrent target-failure/direct-success isolation`
- Final implementation report commit: `cd2c0e39d0b010be51aab3eb7105c1f8b8982da3` (supersedes closure report commit `ee72655147d4374d2d43a08f00b71752a6f55985`)
- Remediation commits: `f91bfa6c43ae98b98680bbbff459cbbb42aa8f3c`, `ec3f1cbb49fdeb08de40113c4ca82af4706f3757`, `9547716b301675244c1ac967088f57832128823a`
- Closure evidence: `Accepted — proxy 21/21, DNS 10/10, TLS 9/9, workspace 55/55, all finding checklists, format, clippy, build, all-target, scope, complete diff, and isolation validated`

**Task Commit Evidence:**

| Task | SHA(s) or justified no-change evidence |
|---|---|
| `P3-G1-T1` | `4b45788c5ce1d4dd2b3490d9accd1fec36a66c1c`, follow-up `3568b1d6c77c651e70f38d4742635aeda9abbdfc` |
| `P3-G1-T2` | `bcf23f409e23294c84565282be19bca2d627375f`, follow-up `3568b1d6c77c651e70f38d4742635aeda9abbdfc` |

#### P3-G2 — listeners-lifecycle-and-cli-composition

- [x] Context and dependency `P3-G1` confirmed.
- [x] Implementation turn completed.
- [x] `P3-G2-T1` — implement-proxy-and-pac-service-loops.
- [x] `P3-G2-T2` — compose-atomic-runtime-and-graceful-shutdown.
- [x] Every task has commit SHA(s) or justified no-change evidence.
- [x] Task-level and group-level checks passed.
- [x] Initial implementation report persisted and separately committed.
- [x] Verification report persisted and separately committed.
- [x] Report status checked and remediation completed or not required.
- [x] Final implementation report commit recorded.
- [x] Orchestrator validated closure, commit evidence, acceptance criteria, and no uncommitted group changes.

**Evidence:**

- Implementation: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p3-g2-implementation-report.md` — `Completed`
- Initial implementation report commit: `6a77c5c15163357e521a5b1a49bd8e121fd63bd4` (supersedes report evidence corrections `f655764385cddb9e0d22bc657c2356beb0f75fbe`, `0a3da17356247a6bd6315e15d4c7d337c8c3d144`)
- Verification: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p3-g2-verification-report.md` — `Passed with no findings`
- Verification report commit: `85db54ec63ad8db354c48a676d5e47a852c1b8ce`
- Remediation: `Not required`
- Final implementation report commit: `6a77c5c15163357e521a5b1a49bd8e121fd63bd4`
- Remediation commits: `None`
- Closure evidence: `Accepted — 63 workspace tests, V14/V16–V19 component evidence, V21 checks, atomic binding, PAC bounds, task ownership, readiness, shutdown, CLI, scope, and isolation passed; Part 4 compiled-process evidence remains planned`

**Task Commit Evidence:**

| Task | SHA(s) or justified no-change evidence |
|---|---|
| `P3-G2-T1` | `f98f6c0794d7738cd9ced28158038f34aa263661`, lint follow-up `140f66d06ba136e89501f18c3f90ce172b1a49bb` |
| `P3-G2-T2` | `8d3efd8c03f29c18639dc49dd356610bf3bdbb76` |

### Part 4 — integration-documentation

- **Plan:** `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-4-integration-documentation.md`
- **Dependencies:** Parts 1–3 completed and accepted
- **Status:** `Completed and accepted`

#### P4-G1 — deterministic-binary-integration

- [x] Context and Parts 1–3 dependencies confirmed.
- [x] Implementation turn completed.
- [x] `P4-G1-T1` — build-authenticated-loopback-test-harness.
- [x] `P4-G1-T2` — prove-direct-selected-and-failure-isolation.
- [x] `P4-G1-T3` — prove-shutdown-readiness-pac-and-secret-safe-outcomes.
- [x] Every task has commit SHA(s) or justified no-change evidence.
- [x] Task-level and group-level checks passed.
- [x] Initial implementation report persisted and separately committed.
- [x] Verification report persisted and separately committed.
- [x] Report status checked and remediation completed or not required.
- [x] Final implementation report commit recorded.
- [x] Orchestrator validated closure, commit evidence, acceptance criteria, and no uncommitted group changes.

**Evidence:**

- Implementation: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p4-g1-implementation-report.md` — `Completed`
- Initial implementation report commit: `06cc3a4372eafaa43feaf5f8b0d90cafb0db9137`
- Verification: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p4-g1-verification-report.md` — `Failed — P4-G1-VER1 race-prone port allocation`
- Verification report commit: `6926ad120347689a6602e3ccce032d318063bcff`
- Remediation: `Completed — kernel-assigned race-free proxy/PAC/origin ports with scoped ephemeral test mode`
- Final implementation report commit: `9e02dfc7b2b3357f168a0aea4d91b490f28543e5`
- Remediation commits: `0ccb9c0fc937817a452dca329bc10b56d8fef6ba`
- Closure evidence: `Accepted — allocation regression 8 starts, five repeated integration suites 40/40, integration 8/8, workspace 72/72, config 6/6, quality/auth/address/safe-port/scope/isolation checks passed`

**Task Commit Evidence:**

| Task | SHA(s) or justified no-change evidence |
|---|---|
| `P4-G1-T1` | `dc846084eb76edc0b3cae53f8b37436d71b1b099` |
| `P4-G1-T2` | `600a73f31dc11c97ebca68a495640299393ea0b2` |
| `P4-G1-T3` | `c14d4fc08f49d3eda78bb7403f4a7d6b4cd60df7` |

#### P4-G2 — example-documentation-and-release-checks

- [x] Context and dependency `P4-G1` confirmed.
- [x] Implementation turn completed.
- [x] `P4-G2-T1` — ship-valid-example-and-operating-guide.
- [x] `P4-G2-T2` — run-release-quality-and-scope-verification.
- [x] Every task has commit SHA(s) or justified no-change evidence.
- [x] Task-level and group-level checks passed.
- [x] Initial implementation report persisted and separately committed.
- [x] Verification report persisted and separately committed.
- [x] Report status checked and remediation completed or not required.
- [x] Final implementation report commit recorded.
- [x] Orchestrator validated closure, commit evidence, acceptance criteria, and no uncommitted group changes.

**Evidence:**

- Implementation: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p4-g2-implementation-report.md` — `Completed`
- Initial implementation report commit: `05ecfb131aee7c68cb632a9639c3036caa28f1ce` (corrected from non-resolving clerical SHA recorded during dispatch)
- Verification: `.s1mple/work/active/w260914ruzu-splithello-start/reports/p4-g2-verification-report.md` — `Passed with no findings`
- Verification report commit: `bf7e8f56b30824b74f1f0f289256f89ecf1ba7dc`
- Remediation: `Not required`
- Final implementation report commit: `05ecfb131aee7c68cb632a9639c3036caa28f1ce`
- Remediation commits: `None`
- Closure evidence: `Accepted — example, README, 73 tests, CLI, V20/V21, dependency/license, security/scope, S1–S17/V1–V21 mapping, T2 no-change, commit scope, and isolation passed`

**Task Commit Evidence:**

| Task | SHA(s) or justified no-change evidence |
|---|---|
| `P4-G2-T1` | `f50c2064d0fc2de16baf2ab8c73a46d87de6a7a3` |
| `P4-G2-T2` | `No change — all required integrated release checks passed; no defect and no empty commit` |

## Roadmap Evidence

Not applicable — standalone work.

## Success Criteria

| Complete | Criterion | Observable condition | Verification cases | Evidence |
|---|---|---|---|---|
| [x] | `S1` | Exact start syntax and valid example reach ready state | `V1`, `V18`, `V20`, `V21` | `Passed — see committed group and overall verification evidence` |
| [x] | `S2` | Invalid or unsafe config fails before listener side effects | `V2`, `V20`, `V21` | `Passed — see committed group and overall verification evidence` |
| [x] | `S3` | Loopback CONNECT establishes only after upstream readiness without root or system mutation | `V5`, `V6`, `V14`, `V18` | `Passed — see committed group and overall verification evidence` |
| [x] | `S4` | PAC routes configured targets to the proxy and other hosts direct | `V7`, `V18` | `Passed — see committed group and overall verification evidence` |
| [x] | `S5` | Domain matching is canonical, label-aware, explicit, and duplicate-safe | `V3`, `V4`, `V7`, `V14`, `V20` | `Passed — see committed group and overall verification evidence` |
| [x] | `S6` | Selected targets resolve only through validated authenticated DoH A answers | `V8`, `V9`, `V15` | `Passed — see committed group and overall verification evidence` |
| [x] | `S7` | DNS cache obeys response/configured TTL and capacity limits | `V10`, `V15` | `Passed — see committed group and overall verification evidence` |
| [x] | `S8` | One bounded complete target ClientHello is recognized across input records | `V11`, `V13`, `V15` | `Passed — see committed group and overall verification evidence` |
| [x] | `S9` | Rewritten output uses valid TLS records split inside matching SNI | `V12`, `V15` | `Passed — see committed group and overall verification evidence` |
| [x] | `S10` | Reassembled output ClientHello handshake bytes exactly equal input | `V12`, `V15` | `Passed — see committed group and overall verification evidence` |
| [x] | `S11` | Origin TLS remains end-to-end and post-prefix bytes remain opaque | `V13`, `V14`, `V15`, `V16` | `Passed — see committed group and overall verification evidence` |
| [x] | `S12` | Listeners, inputs, buffers, DNS, timeouts, ports, and concurrency are bounded | `V2`, `V6`, `V9`, `V13`, `V16`, `V17` | `Passed — see committed group and overall verification evidence` |
| [x] | `S13` | One target failure does not break the listener or an unrelated tunnel | `V14`, `V16` | `Passed — see committed group and overall verification evidence` |
| [x] | `S14` | Startup and shutdown own only process sockets/tasks and complete within deadline | `V17` | `Passed — see committed group and overall verification evidence` |
| [x] | `S15` | Startup and tunnel outcomes are stable and actionable | `V18`, `V19` | `Passed — see committed group and overall verification evidence` |
| [x] | `S16` | Documentation accurately states setup, rollback, and unsupported methods | `V19`, `V20` | `Passed — see committed group and overall verification evidence` |
| [x] | `S17` | Deterministic tests and Rust quality checks cover every design invariant | `V1`–`V21` | `Passed — see committed group and overall verification evidence` |

## Verification Cases

| Complete | Case | Command or method | Expected result | Actual result |
|---|---|---|---|---|
| [x] | `V1` | Focused config test loading representative valid TOML | Expected listeners, DoH values, limits, defaults, and targets materialize | `Passed — see committed group and overall verification evidence` |
| [x] | `V2` | Config invalid-input matrix and startup-side-effect test | Unknown/missing/unsafe fields, listeners, CA, strategy, duplicates, and bounds return exact errors before runtime creation | `Passed — see committed group and overall verification evidence` |
| [x] | `V3` | Domain exact/subdomain/adversarial suffix matrix | Reddit exact/subdomain matches; lookalike and appended suffixes do not | `Passed — see committed group and overall verification evidence` |
| [x] | `V4` | Domain IDNA/case/trailing-dot/invalid/duplicate matrix | Canonical equivalents match and invalid/duplicate values are rejected | `Passed — see committed group and overall verification evidence` |
| [x] | `V5` | CONNECT parser valid request test | Hostname-form HTTP/1.1 CONNECT returns normalized host and allowed port | `Passed — see committed group and overall verification evidence` |
| [x] | `V6` | CONNECT method/authority/port/header boundary matrix | Exact bounded HTTP/error outcomes, no panic, no raw header leakage | `Passed — see committed group and overall verification evidence` |
| [x] | `V7` | PAC renderer behavior test | Exact and enabled subdomains return `PROXY`; lookalikes/unrelated return `DIRECT` | `Passed — see committed group and overall verification evidence` |
| [x] | `V8` | DNS valid matching A-response test | Exact IPv4 candidates and minimum accepted TTL returned | `Passed — see committed group and overall verification evidence` |
| [x] | `V9` | DNS malformed/oversized/mismatched/RCODE/CNAME matrix | Exact `ResolveError`; no unvalidated address crosses boundary | `Passed — see committed group and overall verification evidence` |
| [x] | `V10` | DNS cache controlled-time test | Hit before expiry, miss at/after expiry, TTL ceiling and deterministic capacity behavior | `Passed — see committed group and overall verification evidence` |
| [x] | `V11` | TLS ClientHello one/many input-record matrix | Equivalent parsed handshake, SNI, consumed extent, and SNI range | `Passed — see committed group and overall verification evidence` |
| [x] | `V12` | TLS matching-SNI rewrite test | Two valid records, split inside SNI, reassembled handshake exactly equals input | `Passed — see committed group and overall verification evidence` |
| [x] | `V13` | TLS malformed/truncated/oversized/missing/mismatched matrix and arbitrary-byte no-panic loop | Only defined safe cases pass through; unsafe cases reject precisely; no panic or partial transformed output | `Passed — see committed group and overall verification evidence` |
| [x] | `V14` | `cargo test --test start_proxy` direct-relay scenario | Unselected IPv4 and available IPv6 origins receive byte-identical relay; target path unused | `Passed — see committed group and overall verification evidence` |
| [x] | `V15` | `cargo test --test start_proxy` selected-target scenario | Authenticated local DoH sees A query; origin receives valid split records with identical ClientHello; relay succeeds | `Passed — see committed group and overall verification evidence` |
| [x] | `V16` | Concurrent malformed-target and direct-success integration scenario | Target closes with TLS outcome, direct tunnel succeeds, configured concurrency bound holds | `Passed — see committed group and overall verification evidence` |
| [x] | `V17` | Active-tunnel signal-shutdown integration scenario | Process exits within deadline, listeners close, no persistent state or untracked task remains | `Passed — see committed group and overall verification evidence` |
| [x] | `V18` | Exact example-command and readiness/PAC integration scenario | Binary reaches ready with required fields and PAC serves expected policy | `Passed — see committed group and overall verification evidence` |
| [x] | `V19` | Induced DoH failure and captured diagnostics | Stable `dns-error` plus normalized host; no secret, request, DNS, or ClientHello bytes logged | `Passed — see committed group and overall verification evidence` |
| [x] | `V20` | Production parser plus README/config consistency check | Example parses; exact command, setup, rollback, outcomes, and all exclusions agree | `Passed — see committed group and overall verification evidence` |
| [x] | `V21` | `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo build`; `cargo test --workspace` | Every command exits 0 with no warning or test failure | `Passed — see committed group and overall verification evidence` |

## Overall Verification

Start only after all eight groups are accepted with committed reports and closure evidence.

```text
Fresh s1mple-verifier
    → Passed → project documentation finalization
    → Failed → fresh s1mple-implementer remediates every OVR-VER<number>
             → update affected implementation reports and commit them
             → main orchestrator validates closure without rerunning overall verification
             → project documentation finalization
    → Blocked → mark goal Blocked
```

1. Fill `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/overall-verification-prompt.md`; provide this goal, index, all parts, requirements, design, every group implementation report, every group verification report, and `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/overall-verification-report-template.md`.
2. Spawn one fresh `s1mple-verifier`. It reads all sources and reports, confirms group acceptance and full coverage/integration, runs every check below, performs one substantive pass, writes `.s1mple/work/active/w260914ruzu-splithello-start/reports/overall-verification-report.md`, commits only that report, and returns status plus SHA.
3. Record report status/path/SHA, check results, findings, and blockers. Continue directly on `Passed`.
4. On `Failed`, fill `/Users/hieunguyen/.claude/skills/s1mple-plan/templates/overall-remediation-prompt.md` and spawn one fresh `s1mple-implementer` for all findings with IDs `OVR-VER<number>`. The verifier report and verdict remain unchanged; overall verification is not rerun.
5. The remediation implementer fixes every finding within approved scope, runs affected group and overall checks, commits fixes, appends evidence to each affected implementation report, and commits each updated report separately.
6. The main orchestrator validates every finding's resolution, checks, change SHAs, report SHAs, and clean scoped status. Continue only when no finding, blocker, or required check remains unresolved; otherwise mark the goal `Blocked`.

### Checks

| Complete | Check | Command or method | Expected result | Actual result |
|---|---|---|---|---|
| [x] | Format | `cargo fmt --check` | Exit 0 with no formatting diff | `Passed — see committed group and overall verification evidence` |
| [x] | Lint | `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0 with no warnings | `Passed — see committed group and overall verification evidence` |
| [x] | Build | `cargo build` | Exit 0 for edition-2024 binary | `Passed — see committed group and overall verification evidence` |
| [x] | Unit and integration tests | `cargo test --workspace` | All deterministic tests pass without Internet/root | `Passed — see committed group and overall verification evidence` |
| [x] | CLI contract | `cargo run -- --help` and `cargo run -- start --help` | Required `start --config <PATH>` and no expert packet flags | `Passed — see committed group and overall verification evidence` |
| [x] | Example configuration | Load repository `config.example.toml` through production `Config::load` test | Reddit/Medium, loopback, DoH, strategy, and limits match contract | `Passed — see committed group and overall verification evidence` |
| [x] | Documentation consistency | Compare README command, proxy/PAC setup, rollback, outcomes, and exclusions with executable tests/config | No drift | `Passed — see committed group and overall verification evidence` |
| [x] | Clean-room and excluded surface | Focused search/inspection for transparent interception, raw sockets, firewall/system mutation, `unsafe`, and invalid-cert acceptance | Only approved safe explicit-proxy implementation; prohibited terms only in documented non-goals where applicable | `Passed — see committed group and overall verification evidence` |
| [x] | Working-tree isolation | `git status --short` and task/report commit inspection | Only planned work/reports/docs committed; no scoped uncommitted changes | `Passed — see committed group and overall verification evidence` |

### Evidence

- **Status:** `Failed verdict preserved; all findings remediated and orchestrator-validated`
- **Report:** `.s1mple/work/active/w260914ruzu-splithello-start/reports/overall-verification-report.md`
- **Report commit:** `8d05e8e36796529af117fbc7991dfd55a8bed33e`
- **Findings and owners:** `OVR-VER1 — P4-G1 — Fixed; OVR-VER2 — P4-G2 — Fixed`
- **Remediation status:** `Completed`
- **Remediation commits:** `7ee9ddb9adae488af633be5dcbb175223ae051f4`, `193ec9575da47ebae17c3e23413ea6860cd342d5`, `1d9acbc5733bce2904f940f749d3cd2052f9bef3`
- **Updated implementation report commits:** `P4-G1 a7864af7ee41e9e500e06b330f08b13fed5e66b8`; `P4-G2 01c3bb377a3a20367ea3a73bfa2712a9c6327d4c`
- **Closure evidence:** `OVR-VER1 exact allowed-port membership and compiled 403/no-upstream proof passed; OVR-VER2 bootstrap/authentication documentation and V20 proof passed; integration 9/9, workspace 75/75, format, clippy, build, all-target, CLI, security/scope, dependency/license, and isolation checks passed`
- **Remaining issues:** `None`

## Delivery Acceptance

This is standalone work. After technical verification, delivery acceptance means the implementation, reports, example, README, and project documentation are committed with exact evidence. It does not authorize pushing, releasing, deploying, changing the user's browser/system proxy, or claiming that Reddit/Medium are reachable through every ISP. Any optional real-network observation is manual supplementary evidence and must be reported separately from deterministic acceptance.

## Project Documentation Finalization

Only after every group is accepted and the overall report is `Passed`, or every `OVR-VER<number>` has orchestrator-validated remediation closure:

1. The main orchestrator reads `.s1mple/docs/project.md` completely.
2. Update only facts established by the verified implementation: product purpose/scope, new modules and flows, CLI/config contracts, dependencies, limitations, development setup, test layers, and actual verification evidence. Set **Last Modified** to the execution date.
3. Preserve unrelated guidance and concurrent staged/unstaged work. Validate all relative links, source claims, commands, and documented-versus-executed evidence.
4. Commit only `.s1mple/docs/project.md`. If verified implementation causes no factual change, record the exact no-change reason and inspection evidence; do not create an empty commit.
5. Record validation and commit SHA below. Missing documents, conflicts, failed validation/commit, or uncommitted scoped document changes block completion.

- **Changes or No-change Reason:** `Updated purpose/scope, architecture, modules, CLI/config contracts, dependencies, operating limits, test layers, DoH bootstrap/authentication, scoped test port-0 behavior, and verified evidence from the completed implementation.`
- **Validation and Commit SHA:** `Relative source links and documented commands/evidence validated; committed only .s1mple/docs/project.md at ceedfe9183f057c5e98747738154cfb8d1305ebc.`

**Commit Message:**

```text
docs(project): document SplitHello proxy

Work: W260914RUZU — splithello-start
```

## Implementation Evidence

- **Commits:** `Task, remediation, report, overall-verification, and project-documentation commits are recorded in each group evidence section; final overall remediation commits are 7ee9ddb9adae488af633be5dcbb175223ae051f4, 193ec9575da47ebae17c3e23413ea6860cd342d5, and 1d9acbc5733bce2904f940f749d3cd2052f9bef3; project documentation is ceedfe9183f057c5e98747738154cfb8d1305ebc.`
- **Subagent Replacements:** `None — every required implementer and verifier was resumed in the same role when remediation was needed.`
- **Changed Files:** `Cargo.toml, Cargo.lock, README.md, config.example.toml, src/main.rs, src/cli.rs, src/config.rs, src/domain.rs, src/dns.rs, src/tls.rs, src/proxy.rs, src/pac.rs, src/app.rs, src/diagnostics.rs, tests/start_proxy.rs, tests/support/mod.rs, all required reports, and .s1mple/docs/project.md.`
- **Automated Verification:** `cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings, cargo build, cargo check --all-targets --all-features, cargo test --workspace (75/75), cargo test --test start_proxy (9/9), both CLI help commands, focused V20/config/parser/runtime tests, and repeated port-allocation stress passed.`
- **Manual Evidence:** `Dependency features/licenses, additive CA authentication with negative untrusted client, selected IPv4/no OS fallback, direct IPv4/IPv6 preservation, exact ClientHello handshake bytes, task ownership/shutdown, documentation consistency, prohibited surfaces, commit scope, relative links, and working-tree isolation were inspected and validated.`

## Blockers and Deviations

None.

## Final Completion Checklist

- [x] All four parts and eight groups are completed in order.
- [x] All nineteen planned task outcomes are implemented.
- [x] Every task and remediation change has a SHA or justified no-change evidence; no group implementation changes remain uncommitted.
- [x] Every subagent replacement has a recorded reason and complete same-role handoff.
- [x] Every group has one complete committed implementation report and one complete committed verification report.
- [x] Every group verifier finding, blocker, and required check is resolved with persisted closure evidence.
- [x] Every group acceptance criterion is satisfied.
- [x] `W260914RUZU-REQ1@R1` through `W260914RUZU-REQ9@R1` are covered.
- [x] `W260914RUZU-DES1` through `W260914RUZU-DES7` are covered.
- [x] `S1` through `S17` have passing evidence.
- [x] `V1` through `V21` have passing evidence.
- [x] All seven cross-part contracts and handoffs in the index are integrated and verified.
- [x] All nine overall verification checks pass with actual evidence.
- [x] The overall-verification report is committed and its path/SHA recorded.
- [x] The overall report passed with no findings, or every `OVR-VER<number>` has orchestrator-validated remediation, change SHAs, checks, and updated report SHAs.
- [x] Selected DoH authentication was never disabled; additional CA trust is additive and test evidence includes a negative untrusted-client case.
- [x] Selected resolution remains IPv4-only and never uses OS DNS; unselected direct resolution preserves normal OS IPv4/IPv6 candidates.
- [x] Origin TLS remains end-to-end; no local CA is installed and no application plaintext is captured.
- [x] No `unsafe`, transparent interception, root/firewall/raw-socket/system mutation, packet recipe, target IPv6, or QUIC behavior was added.
- [x] Default tests require neither public Internet nor root and have deterministic timeout/cleanup behavior.
- [x] Project documentation matches the verified implementation and has a final commit SHA or justified no-change evidence.
- [x] No required check, blocker, deviation, report finding, or scoped uncommitted change remains.
- [x] Final changes match the complete approved plan.

## Completion Summary

`Complete — SplitHello now ships the approved safe-Rust loopback CONNECT/PAC application with strict configuration and domain policy, authenticated target-only IPv4 DoH, bounded byte-preserving SNI-internal TLS ClientHello record fragmentation, unchanged direct IPv4/IPv6 relay, isolated tracked lifecycle and shutdown, deterministic authenticated compiled-binary tests, a production-valid Reddit/Medium example, accurate operating documentation, committed group/overall evidence, closed verifier findings, and finalized project documentation. Final automated evidence: 75/75 workspace tests, 9/9 compiled integration tests, format, Clippy with denied warnings, build, all-target check, CLI, V20 consistency, security/scope, dependency/license, and isolation checks passed. No push, release, deployment, system proxy change, or live ISP-access claim was made.`
