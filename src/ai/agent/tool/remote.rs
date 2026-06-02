use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;
use url::Url;

use crate::ai::agent::tool::result::CallOutput;
use crate::ai::agent::tool::result::CallResult;
use crate::ai::agent::tool::result::ExecutionError;
use crate::ai::agent::tool::result::ExecutionResult;
use crate::ai::resolver::tool::{IToolCall, ToolDef};
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
    defination: ToolDef, // TODO: redundant
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
            .map_err(|e| ExecutionError::exec_fail(format!("remote tool request failed: {e:?}")))?;

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

    pub fn tool_definations(&self) -> Vec<ToolDef> {
        let mut definations: Vec<ToolDef> =
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
            defination: ToolDef,
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

    use crate::ai::resolver::tool::ParamDef;

    use std::io::Read;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

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

    fn spawn_json_server(body: String) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("test server local addr");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept test request");
            let mut buffer = [0; 4096];
            let _ = stream.read(&mut buffer);

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write test response");
        });

        Url::parse(&format!("http://{addr}/")).expect("test server url")
    }

    #[tokio::test]
    async fn registers_remote_tools() {
        let server_body = serde_json::json!({
            "tools": [
                {
                    "defination": {
                        "name": "remote_echo",
                        "description": "Echo remote input.",
                        "parameters": {
                            "type": "object",
                            "properties": {},
                            "required": [],
                            "additionalProperties": false
                        },
                        "strict": true
                    },
                    "call_url": "http://127.0.0.1:1/call"
                }
            ]
        })
        .to_string();

        let register_url = spawn_json_server(server_body);
        let mut proxy = RemoteProxy {
            client: HttpClient::new(None),
            tools: HashMap::new(),
        };

        proxy
            .register_tools(&[RemoteServerConfig {
                name: "test".to_string(),
                register_url,
            }])
            .await
            .unwrap();
        let definations = proxy.tool_definations();

        assert_eq!(definations.len(), 1);
        assert_eq!(definations[0].name, "remote_echo");
        assert_eq!(definations[0].strict, Some(true));
    }

    #[tokio::test]
    async fn remote_tool_execute_returns_execution_output_only() {
        let body = serde_json::json!({
            "status": "Success",
            "data": {
                "output": "remote ok"
            }
        })
        .to_string();
        let call_url = spawn_json_server(body);
        let tool = RemoteTool {
            defination: ToolDef::new("remote_echo", "Echo remote input.", ParamDef::new("object")),
            call_url,
        };
        let call = FakeToolCall {
            id: "call-1".to_string(),
            name: "remote_echo".to_string(),
            args: r#"{"text":"hello"}"#.to_string(),
        };

        let output = tool.execute(&HttpClient::new(None), &call).await.unwrap();

        assert_eq!(output, "remote ok");
    }

    #[tokio::test]
    async fn execute_proxy_returns_call_output_with_id() {
        let body = serde_json::json!({
            "status": "Success",
            "data": {
                "output": "remote ok"
            }
        })
        .to_string();
        let call_url = spawn_json_server(body);
        let mut tools = HashMap::new();
        tools.insert(
            "remote_echo".to_string(),
            RemoteTool {
                defination: ToolDef::new(
                    "remote_echo",
                    "Echo remote input.",
                    ParamDef::new("object"),
                ),
                call_url,
            },
        );
        let proxy = RemoteProxy {
            client: HttpClient::new(None),
            tools,
        };
        let call = FakeToolCall {
            id: "call-1".to_string(),
            name: "remote_echo".to_string(),
            args: r#"{"text":"hello"}"#.to_string(),
        };

        let output = proxy.handle_call(&call).await.unwrap();

        assert_eq!(output.call_id, "call-1");
        assert_eq!(output.content, "remote ok");
    }
}
