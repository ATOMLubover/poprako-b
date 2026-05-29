mod poprako_s;

use std::env;
use std::sync::Arc;

use url::Url;

use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::local::memory::{
    GenerateMemoryShardTool, ListMemoryShardsTool, RecallMemoryShardTool,
};
use crate::ai::agent::tool::local::subagent::RunSubagentsTool;
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
            memory_dir,
        )),
    ];

    let base_url = env::var("PRKS_BASE_URL")
        .or_else(|_| env::var("POPRKO_S_BASE_URL"))
        .or_else(|_| env::var("POPRAKO_S_BASE_URL"));
    let qid = env::var("PRKS_QID")
        .or_else(|_| env::var("POPRKO_S_QID"))
        .or_else(|_| env::var("POPRAKO_S_QID"));
    let password = env::var("PRKS_PASSWORD")
        .or_else(|_| env::var("POPRKO_S_PASSWORD"))
        .or_else(|_| env::var("POPRAKO_S_PASSWORD"));

    let (base_url, qid, password) = match (base_url, qid, password) {
        (Ok(base_url), Ok(qid), Ok(password)) => (base_url, qid, password),
        _ => {
            tracing::info!("prks env triple missing, skip prks tools");
            return tools;
        }
    };

    let url = match Url::parse(&base_url).map(normalize_base_url) {
        Ok(url) => url,
        Err(error) => {
            tracing::warn!(error = %error, "invalid PRKS_BASE_URL, skip prks tools");
            return tools;
        }
    };

    let http_client = HttpClient::new(url);
    let token = match PrksClient::login(&http_client, &qid, &password).await {
        Ok(token) => token,
        Err(error) => {
            tracing::warn!(error = %error, "prks login failed, skip prks tools");
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
