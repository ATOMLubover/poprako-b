use std::env;
use std::sync::Arc;

use url::Url;

use crate::ai::agent::IAgentPlugin;
use crate::ai::agent::tool::DynTool;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::message::IMessage;
use crate::bot::agent::tool::poprako_s::GetComicPinnedChapterTool;
use crate::bot::agent::tool::poprako_s::ListChapterAssignmentsTool;
use crate::bot::agent::tool::poprako_s::ListComicChaptersTool;
use crate::bot::agent::tool::poprako_s::ListMyMembersTool;
use crate::bot::agent::tool::poprako_s::ListTeamWorksetsTool;
use crate::bot::agent::tool::poprako_s::ListUserAssignmentsTool;
use crate::bot::agent::tool::poprako_s::ListWorksetComicsTool;
use crate::bot::agent::tool::poprako_s::PrksClient;
use crate::http::HttpClient;

fn normalize_base_url(mut url: Url) -> Url {
    let path = url.path();
    if !path.ends_with('/') {
        url.set_path(&format!("{}/", path));
    }
    url
}

fn build_prks_tools(prks_client: Arc<PrksClient>) -> Vec<DynTool> {
    vec![
        Box::new(ListMyMembersTool::new(prks_client.clone())),
        Box::new(ListTeamWorksetsTool::new(prks_client.clone())),
        Box::new(ListWorksetComicsTool::new(prks_client.clone())),
        Box::new(GetComicPinnedChapterTool::new(prks_client.clone())),
        Box::new(ListComicChaptersTool::new(prks_client.clone())),
        Box::new(ListChapterAssignmentsTool::new(prks_client.clone())),
        Box::new(ListUserAssignmentsTool::new(prks_client)),
    ]
}

pub async fn prks_plugin_from_env() -> PrksPlugin {
    let base_url = env::var("POPRAKO_S_BASE_URL");
    let qid = env::var("POPRAKO_S_QID");
    let password = env::var("POPRAKO_S_PASSWORD");

    let (base_url, qid, password) = match (base_url, qid, password) {
        (Ok(base_url), Ok(qid), Ok(password)) => (base_url, qid, password),
        _ => {
            tracing::info!("POPRAKO_S_* env not set, skip poprako-s tools");
            return PrksPlugin::empty();
        }
    };

    let url = match Url::parse(&base_url).map(normalize_base_url) {
        Ok(url) => url,
        Err(error) => {
            tracing::warn!(error = %error, "invalid POPRAKO_S_BASE_URL, skip poprako-s tools");
            return PrksPlugin::empty();
        }
    };

    let http_client = HttpClient::new(None);
    let token = match PrksClient::login(&http_client, &url, &qid, &password).await {
        Ok(token) => token,
        Err(error) => {
            tracing::warn!(error = %error, "poprako-s login failed, skip poprako-s tools");
            return PrksPlugin::empty();
        }
    };

    PrksPlugin::new(Arc::new(PrksClient::new(http_client, url, token)))
}

pub struct PrksPlugin {
    tools: Vec<DynTool>,
}

impl PrksPlugin {
    pub fn new(prks_client: Arc<PrksClient>) -> Self {
        Self {
            tools: build_prks_tools(prks_client),
        }
    }

    pub fn empty() -> Self {
        Self { tools: Vec::new() }
    }
}

impl<M, R, S, A> IAgentPlugin<M, R, S, A> for PrksPlugin
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    S: Send + Sync + 'static,
    A: Default + Send + Sync + 'static,
{
    fn tools(&mut self) -> Vec<DynTool> {
        std::mem::take(&mut self.tools)
    }
}
