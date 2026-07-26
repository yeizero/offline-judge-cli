# Task 1 implementation report

## Outcome

The Rust-native catalog generator now exposes explicitly typed arguments as
copied `T: Copy` locals and untyped `Display` arguments as borrowed locals.
User expressions are lowered without rewriting declared argument paths, so
rustc owns lexical resolution for ordinary locals, `if let`, `while let`, and
match bindings.

Generated message/schema items live in `#[doc(hidden)]`
`__i18n_schema`; catalog traits and implementations live in
`#[doc(hidden)]` `__i18n_catalog`. Aggregator catalog entries now name modules
such as `en_us`, and routing appends the hidden catalog path internally.
`Catalog`, `CatalogImpl`, and `Localized` work as message keys without changing
the two generic caller forms.

The user expanded the task during implementation to include internal formatter
hygiene. Each generation context now creates one `__i18n_formatter`
`Ident` with `Span::mixed_site()` and reuses it for the formatter declaration
and generated references. User arguments and block locals with the same text
remain distinct and format correctly, including downstream macro expansion.

No feasibility stop condition was reached. Namespace isolation preserves
static const routing, generic locale-only routing, partial fallback, lazy
allocation-free display, and cross-crate caller syntax with small linear
generation changes.

## RED evidence

### Copy bindings and namespaces

Command:

```text
cargo test -p i18n-macro --test runtime -- --nocapture
```

Observed RED:

- E0428: generated message keys `Catalog` and `CatalogImpl` collided with
  protocol items of the same names.
- E0404: the generated `CatalogImpl` message struct displaced the expected
  trait.
- E0573: module-path aggregator entries `en_us` and `zh_tw` were treated as
  catalog types.

Command:

```text
cargo test -p i18n-macro --test ui -- --nocapture
```

Observed RED:

- the new `typed_non_copy_argument.rs` case unexpectedly compiled, proving
  typed `String` had no `Copy` requirement;
- shared runtime fixtures also exposed the expected pre-namespace item
  collisions. Those collateral diagnostics disappeared after isolation and
  were not accepted.

### Formatter hygiene override

Command:

```text
cargo test -p i18n-macro --test runtime formatter_identifier_hygiene_preserves_argument_and_local_captures -- --nocapture
```

Observed RED:

- E0308 at formatter writes: the call-site generated
  `__i18n_formatter` resolved to a user argument/local reference, producing an
  immutable generic reference where `&mut Formatter` was required.

Each test names the production break it catches and exercises the real
generated code. No mocks are used.

## GREEN evidence

After removing AST rewriting, generating typed-copy/untyped-reference
bindings, and isolating namespaces:

```text
cargo test -p i18n-macro --test runtime -- --nocapture
```

Passed 13/13 at that checkpoint, including lexical shadowing, protocol-name
keys, module-path routing, canonical field ordering, fallback, and ownership.

```text
cargo test -p i18n-macro --test catalog_runtime -- --nocapture
cargo test -p i18n-macro --test cross_crate -- --nocapture
```

Passed 3/3 protocol tests and 1/1 downstream fixture.

The new trybuild diagnostic was inspected before acceptance:

```text
error[E0277]: the trait bound `String: Copy` is not satisfied
```

Namespace-only path changes in existing snapshots and the existing
schema-mismatch diagnostic were also inspected before running:

```text
TRYBUILD=overwrite cargo test -p i18n-macro --test ui -- --nocapture
```

The accepted UI suite passed 13/13 cases.

After the formatter-hygiene override:

```text
cargo test -p i18n-macro --test runtime formatter_identifier_hygiene_preserves_argument_and_local_captures -- --nocapture
cargo test -p i18n-macro --test cross_crate -- --nocapture
```

Passed 1/1 locally and 1/1 cross-crate. The complete package suite then passed
13 unit tests, 3 catalog protocol tests, 1 cross-crate fixture, 14 runtime
tests, 13 trybuild cases, and doc tests.

Clippy smoke verification also passed:

```text
cargo clippy --workspace --all-targets -- -D warnings
```

The Windows toolchain emitted only its informational `linker_messages`
warning, which rustc explicitly states ignores `-D warnings`.

## Implementation decisions

- `lower_message_expression` receives only the untouched expression and the
  mixed-site formatter identifier. `ArgumentReferences`, its scope stack, and
  pattern collector were deleted.
- A typed binding calls a small generated `Copy`-bounded helper on the stored
  field reference. This produces a direct rustc E0277 for non-`Copy` types and
  exposes the copied declared type to the expression.
- An untyped binding borrows the stored generic field, retaining `&str` and
  `&&str` behavior.
- Message values and exact length-encoded field markers are emitted beneath
  `__i18n_schema`. `CatalogImpl`, the complete fallback seal, locale
  `Catalog`, and static/dynamic mapping traits are emitted beneath
  `__i18n_catalog`.
- Partial catalog schema paths are rebased once for the generated child
  namespace. Aggregator module paths are similarly rebased and expanded to
  their hidden catalog types.
- The aggregator exposes the fallback hidden schema through its hidden helper,
  while its `Localized` wrapper remains separate. Caller macros construct
  `$crate::__i18n_generated::schema::$message` directly.
- `syn` no longer enables `visit-mut`.
- The generated message struct permits Clippy's `pub_underscore_fields` so a
  valid user argument such as `__i18n_formatter` does not make downstream
  `-D warnings` fail.

## Self-review

- Typed `Option<usize>` tests would fail if typed fields were borrowed instead
  of copied; the `String` UI case would fail to fail if the `Copy` bound were
  removed.
- `if let`, `while let`, match bindings, and ordinary local shadowing are
  resolved exclusively by rustc.
- Heterogeneous untyped canonical-order coverage would fail if generic field
  identity were assigned by locale declaration order.
- Both caller macros exercise all three formerly colliding keys.
- The formatter collision is covered for both a declared argument and a block
  local; the downstream fixture proves exported macro hygiene.
- Aggregator routing still matches only locales, and each caller macro still
  has one static and one dynamic arm.
- No runtime map, allocation, message-key enumeration, public `messages`
  namespace, path duplication, or control-flow rejection policy was added.
- Deferred limitations remain typed borrowed lifetimes, nested-module
  `define_i18n!`, terminal control-flow lowering, and external user
  function/macro expressions.

## Final verification

The exact final gate run passed against the complete implementation:

```text
PASS  cargo fmt --check
PASS  cargo test -p i18n-macro
PASS  cargo test --workspace
PASS  cargo clippy --workspace --all-targets -- -D warnings
PASS  git diff --check
```

The package test gate passed 13 unit tests, 3 catalog protocol tests, 1
cross-crate fixture, 14 runtime tests, 13 trybuild cases, and doc tests. The
workspace gate additionally passed all other workspace tests. The Windows
toolchain emitted only its informational `linker_messages` warning, which
rustc explicitly states ignores `-D warnings`.

## Scope and commit

The diff is limited to `i18n-macro`, its tests/fixtures, the Rust-native
catalog design, this implementation plan, and this report. No file beneath
`shared`, `oj-evaluator`, `oj-generator`, or `editor` is modified.

The implementation commit hash is reported in the task handoff because a
commit cannot contain its own hash.

## Review fix round 1: generated message binding hygiene

Primary review found that a first user argument named `__i18n_message`
shadowed the generated formatter method's message parameter at subsequent
field accesses. The permanent local regression is
`generated_internal_bindings_do_not_capture_user_argument_names`; the
cross-crate provider/consumer fixture covers the same fallback delegation
through an exported caller macro.

Before the production fix, both commands failed:

```text
cargo test -p i18n-macro --test runtime generated_internal_bindings_do_not_capture_user_argument_names -- --nocapture
cargo test -p i18n-macro --test cross_crate -- --nocapture
```

The local and downstream diagnostics were the same exact failure:

```text
error[E0609]: no field `value` on type `&__I18nArgument0`
InternalMessage { __i18n_message, value }
                  --------------  ^^^^^ unknown field
```

`dynamic_trait_item` and `dynamic_override` now each create one
`generated_ident("__i18n_message")` with `Span::mixed_site()` and reuse that
identifier for the formatter method parameter and every generated field
access or fallback-delegation reference.

The adjacent exact-mechanism audit added
`InternalCopy { __i18n_copy, value: usize }`. It passes after the message
parameter fix, demonstrating that the nested copy-helper item is not captured
by an earlier user local. `__i18n_signature` is referenced only in the
delegation method, which creates no user argument bindings, and the formatter
identifier was already mixed-site. No other generated local has this
call-site shadow path.

Fresh review-round verification passed:

```text
PASS  cargo fmt --check
PASS  cargo test -p i18n-macro
PASS  cargo test --workspace
PASS  cargo clippy --workspace --all-targets -- -D warnings
PASS  git diff --check
```

The package gate passed 13 unit tests, 3 catalog protocol tests, 1 cross-crate
fixture, 15 runtime tests, 13 trybuild cases, and doc tests. The only warning
was the previously documented informational Windows `linker_messages`
warning, which rustc explicitly states ignores `-D warnings`.
