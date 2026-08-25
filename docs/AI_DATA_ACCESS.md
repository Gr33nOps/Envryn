# Envryn — AI Data Access

This document is the **register of every AI feature and exactly what data it may see.**

Specification section 59 requires that every AI feature document five things before it is built.
This file is where that requirement is discharged. A feature that is not in the table below
has no approved data access, and the gateway will refuse it.

---

## 1. Exposure levels

| Level | AI receives | Default? |
|---|---|---|
| **0** | No vault data at all. Only the user's typed question. | — |
| **1** | Metadata only: name, type, project, environment, tags, dates. **No values.** | **Yes — the default** |
| **2** | One specific secret value, for one operation, for the shortest possible lifetime. | Ask every time |
| **3** | Several user-selected records, including values. | Ask every time |
| **Forbidden** | Whole decrypted vault, automatically. | **Never implemented** |

The Forbidden level is not a policy that could be relaxed by a setting. There is no code path
that constructs it — see `AI_SECURITY.md` for how that is enforced by the type system.

---

## 2. Feature register

Every row is a contract. Changing any cell requires updating `THREAT_MODEL.md` and re-reviewing.

### Tier 1 (MVP)

**All five implemented and tested** (`crates/envryn-core/src/ai/gateway.rs`'s five typed
methods; `src-tauri/src/ai.rs`'s matching commands; `crates/envryn-core/tests/ai_real_model.rs`
proves three of the five against real local inference -- classification, naming, and search-
intent parsing -- with a real Qwen2-0.5B-Instruct model, including the honest finding recorded
in `AI_SECURITY.md` section 5 about that model's reliability on the five-field search schema
specifically).

| Feature | Level | Input required | Why that input | Plaintext secret? | Persistence | Confirmation |
|---|---|---|---|---|---|---|
| **Secret classification** | 2 | The single pasted value | Prefix and shape determine the type; metadata alone cannot identify an unknown credential | **Yes** | None. Cleared after response. | Suggestion only — user accepts the type |
| **Smart naming** | 2 | The single pasted value + detected provider | Naming convention follows from provider and type | **Yes** | None | User accepts / edits / ignores |
| **`.env` import** | 1 | Variable **names** only, from the deterministic parser | Classification of `DATABASE_URL` follows from the name; the value adds nothing | **No** | None | Preview shown; nothing saved before Import |
| **Structured extraction** | 3 | The pasted block the user explicitly submitted | The block *is* the input; fields must be read out of it | **Yes** | None | Full field preview before save |
| **Natural-language search** | 0 | The search query string | Only the query is parsed into filters | **No** | None | None — read-only; vault engine runs the query |

**Frontend wiring, honestly scoped.** Classification is wired into the create-secret form (a
"Suggest type" action next to the value field, `apps/ui/src/components/envryn/SecretForm.tsx`)
and tries `classify_deterministic` first, falling back to the AI command only if that finds
nothing and local AI is enabled and running -- exactly the "deterministic first" rule in
section 3 below, expressed in the UI's own call order, not only in the backend. Smart naming,
`.env` import, structured extraction, and natural-language search have real, tested backend
commands (`ai_suggest_name`, `ai_classify_env_names`, `ai_extract_structured_fields`,
`ai_parse_search_intent`) but are **not yet wired into their own frontend flows** -- there is no
`.env` import screen, no structured-extraction UI, and the existing search box still does plain
substring filtering rather than calling `ai_parse_search_intent`. Recorded here as real,
scoped-out remaining work, not silently left undone.

### Tier 2 (post-MVP)

| Feature | Level | Input required | Why that input | Plaintext secret? | Persistence | Confirmation |
|---|---|---|---|---|---|---|
| **Semantic duplicates** | 1 | Names, types, projects, environments | Exact duplicates are found by keyed fingerprint in Rust; AI only judges *semantic* similarity | **No** | None | Compare / Ignore. Never auto-deletes |
| **Vault cleanup review** | 1 | Metadata across the vault | The analysis is inherently about metadata | **No** | None | Read-only suggestions |
| **Risk hints** | 1 | Name, type, provider, environment | `SUPABASE_SERVICE_ROLE_KEY` is identifiable by name | **No** | None | Informational only |
| **Explain secret** | 1 | Name and type | The name is sufficient to explain purpose and sensitivity | **No** | None | Read-only |
| **Naming consistency** | 1 | Names across the vault | Comparison is over names | **No** | None | Never renames automatically |
| **Environment comparison** | 1 | Names per environment | Mostly deterministic; AI only explains differences | **No** | None | Read-only |

### Tier 3 (later)

| Feature | Level | Input required | Why that input | Plaintext secret? | Persistence | Confirmation |
|---|---|---|---|---|---|---|
| **Security assistant** | 0 | The question | General knowledge; no vault access | **No** | None | Read-only |
| **Note summarisation** | 3 | The one note the user selected | The note text is the subject | **Yes** (notes may embed credentials) | None | Confirm before any rewrite |
| **Embedded secret detection** | 3 | The one note the user selected | Detection requires reading the text | **Yes** | None | Confirm before extracting to a field |
| **Project templates** | 0 | Project type string | Generates field **names** only | **No** | None | User picks which to create |

---

## 3. Rules that apply to every row

**Deterministic first (spec section 19).** Classification runs a rules engine before the model.
Known prefixes and shapes are matched in Rust — instantly, privately, and with no model
installed. The AI is the *fallback* for values the rules do not recognise, never the primary path.
The same principle governs `.env` parsing (a real parser, not the model) and exact duplicate
detection (keyed HMAC, not the model).

**Values are never displayed by AI output (spec section 24).** Search and analysis results show
name, project, environment, and type. Revealing a value is always a separate, explicit user action.

**Level 2 and 3 show a data-access indicator (spec section 56).** Before plaintext is handed to the
model, the user sees what will be analysed and that it stays on the device. Level 0 and 1
operations show **no** such warning — warning on every metadata operation would train users to
click through, which makes the Level 2 warning worthless.

**Not yet implemented in the UI.** The one Level 2 flow that is wired up today (`SecretForm.tsx`'s
"Suggest type") falls back to `ai_classify_pasted_value` with no visible indicator that the
pasted value is about to be handed to the local model -- it happens silently after deterministic
matching finds nothing. The backend-level guarantees this paragraph describes elsewhere in this
document (bounded, on-device, nothing persisted) all hold regardless; what is missing is
specifically the human-visible "this is about to see your value" moment the spec calls for.
Recorded as real remaining frontend work, not implemented as done.

**Nothing is persisted.** No AI operation writes prompt content, model input, or model output to
disk. AI *suggestions the user accepted* become ordinary vault data and are stored and synced
as such (spec section 53) — but as vault records, not as AI history.

**Budgets are enforced by the gateway, not by the model (spec section 48).** Implemented in
`crates/envryn-core/src/ai/budgets.rs`, checked before anything reaches
[`SanitizedPrompt`](AI_SECURITY.md), and tested
(`gateway::tests::a_value_over_budget_never_reaches_the_engine` asserts the engine is never even
called once a budget is exceeded, not just that the caller sees an error).

| Limit | Value | Status |
|---|---|---|
| Max pasted value (classification, naming) | 4 KiB | Implemented (`MAX_VALUE_BYTES`) |
| Max submitted block (structured extraction) | 32 KiB | Implemented (`MAX_BLOCK_BYTES`) |
| Max `.env` names per import | 512 names, 256 bytes each | Implemented (`MAX_ENV_NAMES`, `MAX_ENV_NAME_BYTES`) -- no separate "256 KiB total import size" budget exists; the per-name-count and per-name-length limits bound it instead |
| Max search query | 512 bytes | Implemented (`MAX_QUERY_BYTES`) -- narrower than "records per Level 3 operation" since there is no `OrganizeSelection`-style multi-record operation in the five built |
| Max prompt length | ~8,192 tokens, approximated as a byte budget (`MAX_PROMPT_BYTES`) | Implemented as an approximation, not an exact count -- this crate holds no tokenizer until a model is loaded; see `budgets.rs`'s own doc comment for why the approximation is deliberately conservative (never permits *more* than the documented token count) |
| Max model response | 1,024 tokens | Implemented (`MAX_RESPONSE_TOKENS`), enforced both as the `max_tokens` passed to the engine and as a hard cap the caller does not exceed regardless of what a misbehaving worker claims to have generated |

Exceeding a budget is a clean refusal with an explanation, never a silent truncation — a silently
truncated `.env` import would drop credentials without telling anyone.

---

## 4. Adding a feature

1. Add a row to the table above **first**.
2. Answer the section 69 checklist in the pull request:
   - Can this be done reliably without AI? *(If yes, do that instead.)*
   - Does the AI need the actual secret value, or would metadata do?
   - Can the input be reduced further?
   - What happens if the model hallucinates?
   - What happens if the input contains prompt injection?
   - Is the output validated against a schema?
   - Does the user confirm mutations?
   - Does it still work offline?
   - Could it leak secret content into logs, cache, or history?
3. Add the `AiOperation` variant and its level policy.
4. Add the tests listed in `AI_SECURITY.md` section "Definition of done".

If step 2 does not have good answers, the feature waits. That is a normal outcome.
