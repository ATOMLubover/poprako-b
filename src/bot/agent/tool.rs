mod poprako_s;

use std::env;
use std::sync::Arc;

use url::Url;

use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::local::memory::{
    GenerateMemoryShardTool, ListMemoryShardsTool, RecallMemoryShardTool,
};
use crate::ai::agent::tool::local::subagent::RunSubagentsTool;
use crate::ai::agent::tool::local::web::WebSearchTool;
use crate::bot::agent::memory_dir;
use crate::bot::agent::tool::poprako_s::{
    GetComicPinnedChapterTool, ListChapterAssignmentsTool, ListComicChaptersTool,
    ListMyMembersTool, ListTeamWorksetsTool, ListUserAssignmentsTool, ListWorksetComicsTool,
    PrksClient,
};
use crate::http::HttpClient;

pub async fn build_tools() -> Vec<DynTool> {
    let memory_dir = memory_dir();
    let mut tools: Vec<DynTool> = vec![
        Box::new(ListMemoryShardsTool::new(memory_dir.clone())),
        Box::new(RecallMemoryShardTool::new(memory_dir.clone())),
        Box::new(GenerateMemoryShardTool::new(memory_dir.clone())),
        Box::new(RunSubagentsTool::new(
            "deepseek-v4-flash".into(),
            5,
            memory_dir.clone(),
        )),
    ];

    if let Some(ws) = WebSearchTool::from_env() {
        tools.push(Box::new(ws));
    } else {
        tracing::info!("TAVILY_API_KEY not set, skip web_search tool");
    }

    let base_url = env::var("POPRAKO_S_BASE_URL");
    let qid = env::var("POPRAKO_S_QID");
    let password = env::var("POPRAKO_S_PASSWORD");

    let (base_url, qid, password) = match (base_url, qid, password) {
        (Ok(base_url), Ok(qid), Ok(password)) => (base_url, qid, password),
        _ => {
            tracing::info!("POPRAKO_S_* env not set, skip poprako-s tools");
            return tools;
        }
    };

    let url = match Url::parse(&base_url).map(normalize_base_url) {
        Ok(url) => url,
        Err(error) => {
            tracing::warn!(error = %error, "invalid POPRAKO_S_BASE_URL, skip poprako-s tools");
            return tools;
        }
    };

    let http_client = HttpClient::new(url);
    let token = match PrksClient::login(&http_client, &qid, &password).await {
        Ok(token) => token,
        Err(error) => {
            tracing::warn!(error = %error, "poprako-s login failed, skip poprako-s tools");
            return tools;
        }
    };

    let prks_client = Arc::new(PrksClient::new(http_client, token));

    tools.push(Box::new(ListMyMembersTool::new(prks_client.clone())));
    tools.push(Box::new(ListTeamWorksetsTool::new(prks_client.clone())));
    tools.push(Box::new(ListWorksetComicsTool::new(prks_client.clone())));
    tools.push(Box::new(GetComicPinnedChapterTool::new(
        prks_client.clone(),
    )));
    tools.push(Box::new(ListComicChaptersTool::new(prks_client.clone())));
    tools.push(Box::new(ListChapterAssignmentsTool::new(
        prks_client.clone(),
    )));
    tools.push(Box::new(ListUserAssignmentsTool::new(prks_client)));

    tools
}

fn normalize_base_url(mut url: Url) -> Url {
    let path = url.path();
    if !path.ends_with('/') {
        url.set_path(&format!("{path}/"));
    }
    url
}
