use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;
use url::Url;

use crate::ai::agent::tool::result::CallOutput;
use crate::ai::agent::tool::result::CallResult;
use crate::ai::agent::tool::result::ExecutionError;
use crate::ai::agent::tool::result::ExecutionResult;
use crate::ai::resolver::tool::{IToolCall, ToolDefination};
use crate::http::HttpClient;

const CONFIG_PATH: &str = "remote_tool.json";

#[derive(Deserialize)]
struct RemoteServerConfig {
    name: String,
    register_url: Url,
}

#[derive(Deserialize)]
struct RemoteToolConfig {
    #[serde(default)]
    servers: Vec<RemoteServerConfig>,
}

async fn load_config() -> anyhow::Result<RemoteToolConfig> {
    let content = tokio::fs::read_to_string(CONFIG_PATH).await?;
    serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse remote tool config: {}", e))
}

struct RemoteTool {
    defination: ToolDefination, // TODO: redundant
    call_url: Url,
}

impl RemoteTool {
    async fn execute<C>(&self, client: &HttpClient, call: &C) -> ExecutionResult
    where
        C: IToolCall,
    {
        #[derive(Serialize)]
        struct RemoteToolPayload<'a> {
            args: &'a str,
        }

        #[derive(Deserialize)]
        #[serde(tag = "status", content = "data")]
        enum RemoteToolExecutionResponse {
            Success { output: String },
            Error { message: String },
        }

        let payload = RemoteToolPayload { args: call.args() };

        let response: RemoteToolExecutionResponse = client
            .post(self.call_url.clone(), &payload, &[], None)
            .await
            .map_err(|e| {
                ExecutionError::exec_fail(format!("remote tool request failed: {:?}", e))
            })?;

        match response {
            RemoteToolExecutionResponse::Success { output } => Ok(output),
            RemoteToolExecutionResponse::Error { message } => {
                Err(ExecutionError::exec_fail(message))
            }
        }
    }
}

/// Proxy entry for remote tools. It loads tool definitions from remote servers and forwards
/// tool calls to them.
pub struct RemoteProxy {
    client: HttpClient,
    tools: HashMap<String, RemoteTool>,
}

impl RemoteProxy {
    pub async fn from_local_config() -> anyhow::Result<Self> {
        let config = load_config().await?;
        if config.servers.is_empty() {
            anyhow::bail!("remote tool config has no servers");
        }

        let mut proxy = Self {
            client: HttpClient::new(None),
            tools: HashMap::new(),
        };

        proxy.register_tools(&config.servers).await?;

        Ok(proxy)
    }

    pub fn tool_definations(&self) -> Vec<ToolDefination> {
        let mut definations: Vec<ToolDefination> =
            self.tools.values().map(|t| t.defination.clone()).collect();
        definations.sort_by(|a, b| a.name.cmp(&b.name));

        definations
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub async fn handle_call<C>(&self, call: &C) -> CallResult
    where
        C: IToolCall,
    {
        let tool = self
            .tools
            .get(call.name())
            .ok_or_else(|| ExecutionError::exec_fail(format!("tool not found: {}", call.name())))?;
        let content = tool.execute(&self.client, call).await?;

        Ok(CallOutput::new(call.id().to_string(), content))
    }

    async fn register_tools(&mut self, servers: &[RemoteServerConfig]) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        struct RemoteToolRegisterResponse {
            tools: Vec<RemoteToolRegisterItem>,
        }

        #[derive(Deserialize)]
        struct RemoteToolRegisterItem {
            defination: ToolDefination,
            call_url: Url,
        }

        for server in servers {
            let response: RemoteToolRegisterResponse = self
                .client
                .get(server.register_url.clone())
                .await
                .map_err(|e| {
                    anyhow::anyhow!("failed to load remote tools from {}: {:?}", server.name, e)
                })?;

            for item in response.tools {
                let name = item.defination.name.clone();
                if name.is_empty() {
                    tracing::warn!(server = %server.name, "remote tool name is empty, skip");
                    continue;
                }

                if self.tools.contains_key(&name) {
                    tracing::warn!(
                        server = %server.name,
                        tool = %name,
                        "duplicate remote tool name, skip"
                    );
                    continue;
                }

                self.tools.insert(
                    name,
                    RemoteTool {
                        defination: item.defination,
                        call_url: item.call_url,
                    },
                );
            }
        }

        if self.tools.is_empty() {
            anyhow::bail!("no remote tools successfully loaded");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::Json;
    use axum::Router;
    use axum::extract::State;
    use axum::routing::{get, post};

    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::oneshot;

    struct FakeToolCall {
        id: String,
        name: String,
        args: String,
    }

    impl IToolCall for FakeToolCall {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn args(&self) -> &str {
            &self.args
        }
    }

    #[derive(Clone)]
    struct TestRemoteServerState {
        call_url: String,
        received_calls: Arc<Mutex<Vec<String>>>,
    }

    struct TestRemoteServer {
        register_url: Url,
        received_calls: Arc<Mutex<Vec<String>>>,
        shutdown: oneshot::Sender<()>,
    }

    #[derive(Deserialize)]
    struct TestRemoteCallPayload {
        args: String,
    }

    async fn register_handler(
        State(state): State<TestRemoteServerState>,
    ) -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "tools": [
                {
                    "defination": {
                        "name": "remote_echo",
                        "description": "Echo remote input.",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "text": {
                                    "type": "string",
                                    "description": "Text to echo."
                                }
                            },
                            "required": ["text"],
                            "additionalProperties": false
                        },
                        "strict": true
                    },
                    "call_url": state.call_url
                }
            ]
        }))
    }

    async fn call_handler(
        State(state): State<TestRemoteServerState>,
        Json(payload): Json<TestRemoteCallPayload>,
    ) -> Json<serde_json::Value> {
        state.received_calls.lock().await.push(payload.args.clone());

        Json(serde_json::json!({
            "status": "Success",
            "data": {
                "output": format!("remote ok: {}", payload.args)
            }
        }))
    }

    async fn spawn_remote_server() -> TestRemoteServer {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server local addr");
        let base_url = format!("http://{}", addr);
        let received_calls = Arc::new(Mutex::new(Vec::new()));
        let state = TestRemoteServerState {
            call_url: format!("{}/call", base_url),
            received_calls: received_calls.clone(),
        };
        let app = Router::new()
            .route("/register", get(register_handler))
            .route("/call", post(call_handler))
            .with_state(state);
        let (shutdown, shutdown_rx) = oneshot::channel();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("run test server");
        });

        TestRemoteServer {
            register_url: Url::parse(&format!("{}/register", base_url)).expect("test register url"),
            received_calls,
            shutdown,
        }
    }

    #[tokio::test]
    async fn loads_config_registers_remote_tools_and_proxies_call() {
        let server = spawn_remote_server().await;
        let config_json = serde_json::json!({
            "servers": [
                {
                    "name": "test",
                    "register_url": server.register_url.as_str()
                }
            ]
        })
        .to_string();
        let config: RemoteToolConfig = serde_json::from_str(&config_json).unwrap();
        let mut proxy = RemoteProxy {
            client: HttpClient::new(None),
            tools: HashMap::new(),
        };

        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].name, "test");

        proxy.register_tools(&config.servers).await.unwrap();
        let definations = proxy.tool_definations();

        assert_eq!(definations.len(), 1);
        assert_eq!(definations[0].name, "remote_echo");
        assert_eq!(definations[0].strict, Some(true));
        assert!(proxy.has_tool("remote_echo"));

        let call = FakeToolCall {
            id: "call-1".to_string(),
            name: "remote_echo".to_string(),
            args: r#"{"text":"hello"}"#.to_string(),
        };

        let output = proxy.handle_call(&call).await.unwrap();

        assert_eq!(output.call_id, "call-1");
        assert_eq!(output.content, r#"remote ok: {"text":"hello"}"#);

        let received_calls = server.received_calls.lock().await;
        assert_eq!(received_calls.as_slice(), [r#"{"text":"hello"}"#]);
        server.shutdown.send(()).expect("shutdown test server");
    }
}
