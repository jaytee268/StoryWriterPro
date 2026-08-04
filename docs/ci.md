# Kontinuity-CI

Für `main` sollten die beiden GitHub-Checks `linux-fast` und
`macos-validation` als erforderliche Statuschecks in den Branch-Schutzregeln
eingetragen werden. Beide Jobs führen dieselbe fachliche Frontend- und
Rust-Prüfkette aus; Linux ist der schnelle Rückmeldepfad, macOS prüft die
Desktop-Zielplattform.

Die Jobs führen `npm ci`, TypeScript-Typecheck, ESLint, Vitest, den Vite-Build,
`cargo fmt --check`, Clippy mit `--all-targets --all-features -- -D warnings`
und `cargo test` aus. Der Workflow setzt keine Codex-Anmeldedaten und startet
keinen Live-Codex-Test. Die Fake-Provider-E2E-Tests sind Teil von Vitest und
werden in jedem normalen CI-Lauf ausgeführt.

Live-Codex-Tests sind bewusst nicht erforderlich. Sie können separat mit
`STORYMEMORY_RUN_CODEX_CONTINUITY_E2E=1` beziehungsweise
`STORYMEMORY_RUN_CODEX_LONGFORM_E2E=1` und einer authentifizierten Codex-CLI
gestartet werden. Ein fehlender Schalter oder eine fehlende Authentifizierung
wird als übersprungener Test ausgegeben, nicht als erfolgreicher E2E-Nachweis.
