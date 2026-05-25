run-napcat:
	docker compose up -d napcat

run-bot:
	docker compose up -d bot

run-bot-debug:
	RUST_LOG=debug docker compose up -d bot

run-stack:
	docker compose up -d napcat bot

build-bot:
	DOCKER_BUILDKIT=0 docker compose build bot

rebuild-bot: build-bot run-bot-debug

down-stack:
	docker compose down
