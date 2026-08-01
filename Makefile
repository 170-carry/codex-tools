
dev-start:
	cargo run --manifest-path src-tauri/Cargo.toml --bin codex-tools-proxyd


start:
	@trap 'kill 0' INT TERM EXIT; \
	bun run dev & \
	./app

dd:
	@exec ./damon-codex.sh

dev:
	pnpm run tauri dev

codex-start:
	@exec ./scripts/codex-service.sh start

codex-stop:
	@exec ./scripts/codex-service.sh stop

codex-restart:
	@exec ./scripts/codex-service.sh restart

codex-status:
	@exec ./scripts/codex-service.sh status

codex-tail:
	@exec ./scripts/codex-service.sh tail

codex-logs:
	@exec ./scripts/codex-service.sh logs "$(or $(LINES),100)"

codex-install:
	@exec ./scripts/codex-service.sh install

codex-uninstall:
	@exec ./scripts/codex-service.sh uninstall

codex-service-status:
	@exec ./scripts/codex-service.sh service-status

codex-service-restart:
	@exec ./scripts/codex-service.sh service-restart
