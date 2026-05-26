run-napcat:
	docker compose up -d lb-alpine

run-bot:
	docker compose up -d --build bot

run-stack: run-bot run-bot
    @echo "Running the entire stack and the bot..."

run-bot-debug:
	RUST_LOG=debug docker compose up -d --build bot

down-stack:
	docker compose down
