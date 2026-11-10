# v3.2.0 validation

Includes Claude account management and authorization-app selection, the shared desktop UI and responsive account inspector, plus the current local project/session preservation changes.

- Rust library: 479 passed, run with `scripts/test-isolated.sh` after compilation.
- Node 24 unit tests: 59 passed. The system Node 20 cannot execute the TypeScript tests; release CI uses Node 22.
- ESLint, TypeScript/Vite build and 82 page/layout checks: passed.
- Browser checks with synthetic data: 104 layout/interaction checks and 17 dialog/feedback checks passed.
- CLI smoke: 24 synthetic projects, 81 links and 4 unassigned conversations; unresolved references and absent thread index rows preserved. Dry-run and route rejection left source files unchanged.
- Packaged standalone proxy: `cargo check --locked` passed.
- Database tests use small temporary fixtures. No real account database was queried or modified by this validation.
- Existing unused-code and Vite chunk-size warnings remain.

Native Windows behavior, real OAuth provider acceptance and real-account switching were not exercised locally. Release CI must validate macOS and Windows and produce the signed updater artifacts for Apple Silicon, Intel and Windows. This release publishes desktop packages; npm publication is disabled.
