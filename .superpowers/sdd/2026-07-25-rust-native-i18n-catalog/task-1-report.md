# Task 1 implementation report

## Outcome

The stable workspace toolchain supports all five feasibility requirements. The
text-file prototype has been replaced by Rust-native `catalog!` and
`define_i18n!` procedural macros with typed message values, per-message trait
fallback, generic locale routing, const explicit-locale static routing, and two
generic caller arms per exported macro.

No stop condition was reached. The implementation adds no catalog path
duplication, public `messages` module, runtime type erasure, message-specific
caller-arm enumeration, or permanent control-flow rejection policy.

Locale argument declaration order was clarified during review as authoring
order rather than schema identity. Schema-facing arguments are therefore
canonicalized by exact identifier text. Runtime coverage reverses locale
declarations for both distinct typed fields and heterogeneous untyped fields.

## Feasibility and TDD evidence

1. Initial runtime RED:
   `cargo test -p i18n-macro --test runtime -- --nocapture` failed because the
   old aggregator rejected `locale: pub Locale`, `catalog`/`tr`/`tr_for` were
   unresolved, and the old API expected path-based text catalogs.
2. Throwaway handwritten feasibility module GREEN:
   `cargo test -p i18n-macro --test feasibility -- --nocapture` passed 5/5:
   generic associated-constant const routing, partial static/dynamic fallback,
   generic message-agnostic `Localized<M>` routing, `&str`/`&&str` untyped
   rendering, and match-local capture. The throwaway test was deleted after the
   production protocol replaced it.
3. Parser RED/GREEN:
   focused unit tests first failed on missing `CatalogInput`/`CatalogKind`, then
   passed for fallback/schema headers, static/dynamic entries, typed/untyped
   arguments, native `syn::Expr`, duplicates, and malformed input.
4. Lowering RED/GREEN:
   focused tests first failed on missing lowering, then passed for literals,
   blocks, `if`/`else`, `match`, local capture, repeated capture, format specs,
   and escaped braces. A later local-shadowing runtime RED produced E0614 from
   incorrectly rewriting the shadowed integer; scope-aware rewriting made it
   GREEN.
5. Catalog protocol RED/GREEN:
   `tests/catalog_runtime.rs` first failed on missing generated traits/types,
   then passed fallback static/dynamic, typed/native conditions, untyped owned
   generics, match capture, partial omission, and explicit-empty behavior.
6. Locale/router RED/GREEN:
   runtime coverage now passes generated derives, const static routing, current
   locale queried exactly once, explicit locale queried zero times, typed
   `if`/`match`, lazy allocation-free display, ordinary ownership, and
   `&str`/`&&str`.
7. Caller diagnostics RED/GREEN:
   trybuild coverage passes duplicate/unknown keys, missing/extra/duplicate
   fields, static/dynamic misuse, invalid format capture, schema mismatch, and
   a deliberately knowledgeable manual self-fallback attempt.
8. Hygiene RED/GREEN:
   the provider/consumer fixture passes with private real catalog modules and
   downstream provider-qualified `tr!`/`tr_for!` calls.
9. Review RED/GREEN:
   a field-marker identity test reproduced the structural collision
   `A_B`/`C` versus `A`/`B_C`. Length-encoded exact key/field identifiers made
   the test GREEN while retaining linear generation. Case-distinct static and
   dynamic keys also pass.

## Architecture and changed areas

- `src/catalog.rs`: parses real fallback/schema declarations and native Rust
  expressions.
- `src/input.rs`: parses generated-locale aggregator declarations.
- `src/lower.rs`: performs direct formatter lowering and preserves native
  expression structure and lexical bindings.
- `src/generate.rs`: emits the schema protocol, sealed complete fallback,
  partial implementations, locale router, localized display wrapper, and
  generic caller macros.
- Runtime, protocol, UI, and cross-crate fixtures were replaced with native
  catalogs. Obsolete text catalogs and their source-text diagnostics were
  deleted.
- `syn` enables `visit-mut` for scope-aware declared-argument rewriting.

Generated routing is linear: the aggregator emits locale-only matches, while
`tr!` and `tr_for!` each contain exactly one static and one dynamic arm. Rust
named-struct construction supplies caller field validation without enumerating
argument modes.

## Macro hygiene and validation notes

- Exported caller macros use `$crate::__i18n_generated` so downstream
  invocations resolve provider internals. The implementation module and
  reexports are `#[doc(hidden)] pub`; there is no public `messages` namespace.
- Partial-locale method signatures carry a zero-sized canonical schema
  signature. Exact, length-encoded field markers and declared types diagnose
  wrong names/types without rereading source files.
- A private `CompleteCatalog` bound permits generated partial fallback only to
  the complete schema catalog and rejects manual self-fallback cycles even when
  the hidden required const is supplied.
- Relative aggregator paths are rebased from the generated child module.
- The implementation deliberately leaves `return`, `break`, `continue`, `?`,
  and related unsupported lowering boundaries to Rust type checking. It does
  not establish a macro rejection policy. External user function/macro support
  remains deferred by the design.

## Final verification

All required final gates passed on 2026-07-25:

```text
cargo fmt --check                                      PASS
cargo test -p i18n-macro                              PASS
cargo test --workspace                                PASS
cargo clippy --workspace --all-targets -- -D warnings PASS
git diff --check                                       PASS
```

The package gate passed 13 unit tests, 3 catalog protocol tests, 1 cross-crate
fixture, 12 runtime tests, 12 trybuild cases, and doc tests. The Windows
toolchain emitted its existing informational `linker_messages` warning; rustc
explicitly notes that this lint ignores `-D warnings`, and clippy exited zero.

## Scope and commits

The change is confined to `i18n-macro`, its fixtures, and this report. No file
beneath `shared`, `oj-evaluator`, `oj-generator`, or `editor` changed. The
implementation commit hash is reported in the task handoff because a commit
cannot contain its own hash.
