# Public short ids

## Rule

Entities exposed in user-facing URLs carry a **public short id** alongside their
internal `Id<T>` (UUIDv7). The short id is a **stored opaque alias** — an
**11-character Crockford Base32** code with a `UNIQUE` index — not an encoding of
the UUID. URLs and public API surfaces use the short id; the database primary key
and all internal code stay on `Id<T>` ([data-persistence.md](data-persistence.md)).
Resolution is a single indexed lookup at the HTTP boundary.

```
/subscriptions/019e61e2-0a78-7540-b060-0e6e01d171e0   internal / DB
/subscriptions/9TXK4P2RQ8M                            public
```

## Why a stored alias, not an encoding

A UUIDv7 is 128 bits; an 11-char code holds 55. You cannot reversibly shrink one
into the other, so the short id must be a **separate value** with its own column and
unique index. Encoding schemes (Sqids/Hashids, Feistel) need a small sequential
integer key — our entities have none, they're UUIDv7 by standard. The short id is
minted at insert, immutable, and 1:1 with the entity for its lifetime.

## Not a secret

The short id is a **handle, not a capability**. Authorization is enforced
server-side on every request exactly as before ([auth-integration.md](auth-integration.md));
knowing a short id grants nothing. Never gate access on the short id alone. If a
resource is protected *only* by an unguessable URL, use a full-entropy token — not
this mechanism.

## Public is the only public identifier

The public id is the **only** identifier that crosses the public boundary. Anywhere
an id is exposed — URL path, query param, JSON body, real-time event envelope — it is
the public id, never the UUID. Translation lives in the **exposer layer** (the poem
API, and any other outward-facing surface): it maps an inbound `PublicId<T>` to its
`Id<T>` on the way in, and `Id<T>` back to `PublicId<T>` on the way out. `core`, `db`,
and `engine` only ever see `Id<T>` — they never learn an entity has a public id. A
DTO's `id` field therefore carries the public id; the UUID is never serialized to a
client.

## Which entities get one

A **judgement call**: any entity a user might land on, link to, or reference by URL.
Internal-only entities (never addressed from outside the exposer layer) keep just their
`Id<T>` and need no public id.

## Alphabet & length

- **Crockford Base32**, uppercase: `0-9 A-Z` minus `I L O U`. Case-insensitive,
  URL-safe, no ambiguous characters, survives being read aloud or transcribed into a
  support ticket.
- **Fixed 11 chars = 55 bits (~3.6e16 codes).** Generate 7 random bytes (56 bits),
  Crockford-encode, take 11 chars. Insert against the `UNIQUE` index; on conflict,
  regenerate. A collision is a transparent retry, not a failure.
- Normalize on input before lookup: uppercase, then map Crockford's confusables
  (`I/i/L/l → 1`, `O/o → 0`).

### Collision headroom

At 55 bits a collision is a non-event until table sizes far beyond fleet scale. The
`UNIQUE` index + regenerate-on-conflict means a collision only ever costs one extra
insert attempt:

| Rows in table | Per-insert collision probability | Lifetime chance of *any* collision |
|---|---|---|
| 1M | 2.8e-11 | ~0.001% |
| 10M | 2.8e-10 | ~0.14% |
| 100M | 2.8e-9 | ~13% (≈ one transparent retry) |

The ~50% point for a single lifetime collision is ~224M rows. Treat 11 as fixed; if
an entity ever genuinely approaches that scale, widen its column rather than shorten
anything.

## Type-safety

Mirror `Id<T>`: a `PublicId<T>` newtype tagged with the entity, so a subscription's
public id can't be used to look up an order. Runtime representation is 11 ASCII bytes
(`Copy`); it serializes as the bare code string and `parse` normalizes confusable
spellings on the way in. Internals never touch it — resolve to `Id<T>` at the edge.

```rust
// core crate, alongside Id<T>
pub struct PublicId<T> {
    code: [u8; 11],                  // canonical uppercase Crockford Base32
    _marker: PhantomData<fn() -> T>,
}
```

Template: [`templates/rust/public_id.rs`](../templates/rust/public_id.rs) — minting,
Crockford encode/decode, confusable folding, `FromStr`, `Display`, and serde.

## Persistence & resolution

- Column `public_id TEXT NOT NULL UNIQUE` on each public-facing entity. The primary
  key stays the UUIDv7; the unique index is the only thing the public lookup touches.
- One repo method per entity: `Db::resolve_subscription(&PublicId<Subscription>)
  -> Result<Id<Subscription>>` (or fetch the row directly). Map a miss to the standard
  not-found error ([error-handling.md](error-handling.md)).

## Logging

Log **both**: the short id (what appears in user-facing URLs and support tickets) and
the internal `Id<T>` (what joins across services). Short ids aren't secrets, so
they're safe as structured fields ([observability.md](observability.md)).

## Checklist

- [ ] Public-facing entities have `public_id TEXT NOT NULL UNIQUE`; PK stays UUIDv7.
- [ ] Short id is a stored alias: 11-char Crockford Base32, minted at insert,
      regenerated on `UNIQUE` conflict.
- [ ] `PublicId<T>` newtype; resolved to `Id<T>` once, at the HTTP boundary.
- [ ] Input normalized (uppercase + Crockford confusables) before lookup.
- [ ] Authz enforced server-side regardless of the short id — it is not a secret.
- [ ] Public id is the only id exposed; `Id<T>`↔`PublicId<T>` translation lives in the
      exposer layer; `core`/`db`/`engine` see only `Id<T>`.
