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
napcat-host-run:
	docker compose -f docker-compose.host.yml up -d

napcat-host-down:
	docker compose -f docker-compose.host.yml down

napcat-logs:
	docker compose -f docker-compose.host.yml logs -f

# Run poprako-b directly on the host (uses .env).
# Requires: napcat-up has been run and NapCat reverse WS
# configured to point at ws://127.0.0.1:8081/onebot/v11.
bot-host-run:
	mkdir -p logs
	if [ -f .bot-host.pid ] && kill -0 "$(cat .bot-host.pid)" 2>/dev/null; then echo "bot is already running with pid $(cat .bot-host.pid)"; exit 1; fi
	: > logs/bot-host-run.log
	cargo build --release --bin poprako-b-preview >> logs/bot-host-run.log 2>&1
	nohup target/release/poprako-b-preview >> logs/bot-host-run.log 2>&1 & echo $! > .bot-host.pid
	echo "bot started with pid $(cat .bot-host.pid); logs: logs/bot-host-run.log"

bot-host-down:
	if [ ! -f .bot-host.pid ]; then echo "bot is not running"; exit 0; fi
	if kill -0 "$(cat .bot-host.pid)" 2>/dev/null; then kill "$(cat .bot-host.pid)"; else echo "bot pid $(cat .bot-host.pid) is not running"; fi
	rm -f .bot-host.pid

bot-host-run-debug:
	mkdir -p logs
	if [ -f .bot-host.pid ] && kill -0 "$(cat .bot-host.pid)" 2>/dev/null; then echo "bot is already running with pid $(cat .bot-host.pid)"; exit 1; fi
	: > logs/bot-host-run.log
	cargo build --bin poprako-b-preview >> logs/bot-host-run.log 2>&1
	nohup env RUST_LOG=debug target/debug/poprako-b-preview >> logs/bot-host-run.log 2>&1 & echo $! > .bot-host.pid
	echo "bot started with pid $(cat .bot-host.pid); logs: logs/bot-host-run.log"

chatbox-server-run:
    cargo run --bin chatbox
