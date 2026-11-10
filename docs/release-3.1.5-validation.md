# v3.1.5 validation

PR #247 was reviewed and supplemented with a compatibility fix for historical
project mappings and stale sidebar records. A synthetic regression first
reproduced `metadata references an unknown project`; it passes after the fix.
Unresolved records stay with their owner, and collisions with incoming projects
remain errors.

- Rust library: 456 passed in the isolated runner.
- Node 22 unit tests: 59 passed.
- ESLint, TypeScript/Vite build, 44 page checks and 25 layout checks: passed.
- CLI synthetic dry-run: 24 projects, 81 links, 4 unassigned conversations;
  source files unchanged after preview and rejected routes/cloud bindings.
- Packaged standalone proxy: `cargo check --locked` passed.
- Existing unused-code and Vite chunk-size warnings remain.

Compilation ran outside the test sandbox because this machine's Xcode compiler
is under `/Applications`, which the isolated runner deliberately blocks. The
compiled Rust tests and CLI smoke test ran inside the sandbox. The CLI test used
the Command Line Tools Python executable for the same reason. Database tests use
small synthetic databases; project validation uses read-only temporary copies
and keyed lookups, without modifying the user's source SQLite database.

No real account switch or native Windows lifecycle acceptance was performed.
Release CI must validate macOS and Windows and build all desktop installers.
