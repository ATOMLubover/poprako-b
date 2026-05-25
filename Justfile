run-napcat:
	docker compose up -d napcat

run-bot:
	docker compose up -d --build bot

run-bot-debug:
	RUST_LOG=debug docker compose up -d --build bot

down-stack:
	docker compose down
