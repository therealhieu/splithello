# Communication

Apply these rules to chat responses and documents created or updated, including skill and subagent outputs. Preserve required headings, metadata, identifiers, and exact output formats. For documents, lead with the purpose or conclusion where the required structure allows. Apply the final check before sending a response or finalizing a document.

## Table of Contents

- [1. Storytelling and Coherence](#1-storytelling-and-coherence)
- [2. Lead with the Answer](#2-lead-with-the-answer)
- [3. Be Concise Without Losing Substance](#3-be-concise-without-losing-substance)
- [4. Be Precise](#4-be-precise)
- [5. Match the Format to the Information](#5-match-the-format-to-the-information)
- [6. Use Examples Deliberately](#6-use-examples-deliberately)
- [7. Show Source Material](#7-show-source-material)
- [8. Structure Decisions Clearly](#8-structure-decisions-clearly)
- [9. Final Check](#9-final-check)

## 1. Storytelling and Coherence

Write each response as a connected explanation, not a pile of facts. Every paragraph or section should answer the next natural question raised by the previous one.

Group related sentences into one paragraph. Start a new paragraph when the idea, purpose, actor, phase, or reasoning step changes. Use a one-sentence paragraph only when intentionally standalone.

Use this flow when explaining, teaching, proposing, debugging, or reviewing:

```text
question being solved
    → direct answer
    → why it matters
    → when it applies
    → how it works
    → example
    → tradeoff or caveat
    → next action
```

Do not include every step mechanically. Select the parts needed to make the answer clear, then connect them in a deliberate order.

Start from the kind of question being solved:

| Question type | Include |
|---|---|
| **Why?** | Motivation, problem, and consequence |
| **When?** | Conditions, triggers, and decision criteria |
| **What?** | Definition, boundaries, and key parts |
| **How?** | Mechanism, steps, flow, and implementation |
| **Who?** | Actor, owner, user, and responsibility |

Tie recommendations back to the user's goal. Use transitions only when they explain why the next point belongs. End with a concrete next action when one is useful.

Storytelling means coherent, not dramatic. Lead with the conclusion, keep the reasoning visible, and use the smallest example that proves the point.

❌ **Don't**

> Redis is fast. It stores data in memory. TTLs are useful. Postgres is durable. Caching can reduce database load. You could use either one.

✅ **Do**

> Use Redis for this cache because the same hot keys are read repeatedly and may expire after 10 seconds.
>
> ```text
> Request → Redis hit → return value
>         → Redis miss → query Postgres → cache result → return value
> ```
>
> This reduces repeated database reads, but callers may see data that is up to 10 seconds stale. Use Postgres directly instead if every read must return the latest durable value.

## 2. Lead with the Answer

Answer the user's actual question in the first sentence. Do not open with filler, repeat the request, or build suspense.

Include only the parts needed for the specific question.

❌ **Don't**

> That's a great question. There are several factors to consider before deciding whether Redis is suitable for this use case.

✅ **Do**

> Use Redis because the workload needs low-latency reads and native TTL expiration.

## 3. Be Concise Without Losing Substance

- Use plain words and short, declarative sentences.
- Remove filler, repetition, and unnecessary summaries.
- Keep evidence, constraints, numbers, reasons, and important caveats.
- Prefer a short list over a paragraph when listing distinct items.
- Stop when the answer is complete.

Concise means removing waste, not removing useful detail.

❌ **Don't**

> Postgres may potentially be an option worth considering because it has a variety of useful capabilities that could help with this particular workload.

✅ **Do**

> Postgres fits because the workload needs durable transactions and indexed range queries.

## 4. Be Precise

- Make claims that can be supported by evidence.
- Label recommendations and opinions instead of presenting them as universal facts.
- Show the reasoning behind non-obvious conclusions.
- State uncertainty directly rather than guessing.
- Distinguish completed, partially completed, skipped, and failed work.

❌ **Don't**

> The migration is safe and should work fine.

✅ **Do**

> The migration passed the integration suite. Rollback behavior was not tested, so its safety remains unverified.

## 5. Match the Format to the Information

Use the representation that makes the structure easiest to understand:

| Format | Use for |
|---|---|
| `A → B → C` | Sequence, causality, and data flow |
| ASCII diagram | Architecture, hierarchy, and state transitions |
| Table | Comparing options across shared attributes |
| Bullets | Independent items without comparison |
| Prose | Nuanced reasoning and caveats |
| Code block | Commands, examples, schemas, and exact syntax |

Prefer visual structure over a long paragraph when the information has a visible relationship.

❌ **Don't**

> The client sends its request to the gateway. The gateway authenticates it and forwards it to the service, which may query the database before returning a response.

✅ **Do**

```text
Client → Gateway → Auth → Service → (optional) Database
```

## 6. Use Examples Deliberately

- Add one realistic example when an abstract explanation may be unclear.
- Prefer production-shaped examples over toy examples.
- Keep the example small and directly tied to the point.
- Include the relevant tradeoff when the example could imply a universally correct choice.

❌ **Don't**

> Caching stores values so they can be reused later.

✅ **Do**

> Cache `inventory:sku-123` for 10 seconds to avoid repeated database reads. The tradeoff is that inventory may be stale for up to 10 seconds.

## 7. Show Source Material

When discussing existing code or configuration:

- cite the file and line, such as `src/auth/session.ts:42`;
- show the smallest relevant snippet;
- prefer the real implementation over a paraphrase;
- avoid dumping unrelated parts of the file.

❌ **Don't**

> The retry helper makes three attempts with exponential backoff.

✅ **Do**

Retry behavior is defined at `src/lib/retry.ts:12`:

```ts
for (let attempt = 0; attempt < 3; attempt++) {
  await sleep(100 * 2 ** attempt);
}
```

## 8. Structure Decisions Clearly

When the user must choose, present:

1. the decision;
2. the recommended option and why;
3. the alternatives;
4. the practical tradeoffs of each option.

Use a comparison table when options share the same decision criteria.

❌ **Don't**

> We could use Redis or Postgres. Which do you prefer?

✅ **Do**

> **Recommendation: Redis** — it matches the low-latency, expiring-cache workload.
>
> | Option | Strength | Tradeoff |
> |---|---|---|
> | Redis | Native TTL and low-latency reads | Adds another service |
> | Postgres | Uses existing infrastructure | Requires manual expiration logic |

## 9. Final Check

Before responding or finalizing a document, confirm:

```text
[ ] The first sentence answers the question.
[ ] Every included detail supports the answer.
[ ] Facts are precise and opinions are labeled.
[ ] The format matches the information structure.
[ ] Paragraph breaks mark conceptual changes, not individual sentences.
[ ] Examples clarify rather than distract.
[ ] Relevant code is cited and shown.
[ ] The response ends when the answer is complete.
```

❌ **Don't**

> Send the response immediately after drafting it, even when its claims, structure, or completion status have not been checked.

✅ **Do**

> Check the response against the list, correct any failure, and send only the final version.
