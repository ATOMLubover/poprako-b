# # ---- Docker (legacy, docker-compose.bak.yml) ----
# run-napcat:
# 	docker compose -f docker-compose.bak.yml up -d lb-alpine
#
# run-bot:
# 	docker compose -f docker-compose.bak.yml up -d --build bot
#
# run-bot-debug:
# 	RUST_LOG=debug docker compose -f docker-compose.bak.yml up -d --build bot
#
# down-stack:
# 	docker compose -f docker-compose.bak.yml down

# ---- Host mode (docker-compose.host.yml) ----
run-napcat-host:
	docker compose -f docker-compose.host.yml up -d

down-napcat-host:
	docker compose -f docker-compose.host.yml down

napcat-logs:
	docker compose -f docker-compose.host.yml logs -f

# Run poprako-b directly on the host (uses .env).
# Requires: napcat-up has been run and NapCat reverse WS
# configured to point at ws://127.0.0.1:8081/onebot/v11.
run-bot-host:
	cargo run --release

run-bot-host-debug:
	RUST_LOG=debug cargo run

run-chatbox-server:
    cargo run --bin chatbox
