run-napcat:
	docker compose up -d napcat

run-bot:
	docker compose up -d --build bot

run-stack:
	docker compose up -d --build napcat bot
