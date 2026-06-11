pub mod client;
pub use client::PrksClient;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ai::agent::tool::ITool;
use crate::ai::agent::tool::result::{ExecutionError, ExecutionResult};
use crate::ai::resolver::tool::{ParamDef, PropDef, ToolDefination};

fn default_offset() -> i64 {
    0
}

fn default_limit() -> i64 {
    20
}

fn parse_json_args<T: for<'de> Deserialize<'de>>(args: &str) -> Result<T, ExecutionError> {
    serde_json::from_str(args)
        .map_err(|e| ExecutionError::args_schema(format!("invalid args json: {}", e)))
}

fn pretty_json<T: Serialize>(v: &T) -> ExecutionResult {
    serde_json::to_string_pretty(v)
        .map_err(|e| ExecutionError::exec_fail(format!("failed to serialize output: {}", e)))
}

fn workflow_phase_enum() -> Option<Vec<f64>> {
    Some(vec![0.0, 1.0, 2.0])
}

#[derive(Deserialize)]
struct PagingArgs {
    #[serde(default = "default_offset")]
    offset: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}

pub struct ListMyMembersTool {
    prks_client: Arc<PrksClient>,
}

impl ListMyMembersTool {
    pub fn new(prks_client: Arc<PrksClient>) -> Self {
        Self { prks_client }
    }
}

#[async_trait::async_trait]
impl ITool for ListMyMembersTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object").with_properties(vec![
            (
                "offset",
                PropDef::Number {
                    desc: "Pagination offset, default 0".to_string(),
                    r#enum: None,
                },
            ),
            (
                "limit",
                PropDef::Number {
                    desc: "Pagination limit, default 20".to_string(),
                    r#enum: None,
                },
            ),
        ]);

        ToolDefination::new(
            "list_my_members",
            "List current account memberships. includes is hardcoded to include team.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let args: PagingArgs = parse_json_args(args)?;
        let data = self
            .prks_client
            .list_my_members(args.offset, args.limit)
            .await
            .map_err(ExecutionError::exec_fail)?;

        pretty_json(&data)
    }
}

#[derive(Deserialize)]
struct TeamWorksetArgs {
    team_id: String,
    #[serde(default = "default_offset")]
    offset: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}

pub struct ListTeamWorksetsTool {
    prks_client: Arc<PrksClient>,
}

impl ListTeamWorksetsTool {
    pub fn new(prks_client: Arc<PrksClient>) -> Self {
        Self { prks_client }
    }
}

#[async_trait::async_trait]
impl ITool for ListTeamWorksetsTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "team_id",
                    PropDef::String {
                        desc: "Target team id".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "offset",
                    PropDef::Number {
                        desc: "Pagination offset, default 0".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "limit",
                    PropDef::Number {
                        desc: "Pagination limit, default 20".to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec!["team_id".to_string()]);

        ToolDefination::new(
            "list_team_worksets",
            "List worksets under one team.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let args: TeamWorksetArgs = parse_json_args(args)?;
        let data = self
            .prks_client
            .list_team_worksets(&args.team_id, args.offset, args.limit)
            .await
            .map_err(ExecutionError::exec_fail)?;

        pretty_json(&data)
    }
}

// TODO: named phases(i.e. pending/ongoing/completed).
#[derive(Deserialize)]
struct WorksetComicsArgs {
    workset_id: String,
    #[serde(default)]
    fuzzy_title: Option<String>,
    #[serde(default)]
    upload_phase: Option<i64>,
    #[serde(default)]
    translate_phase: Option<i64>,
    #[serde(default)]
    proofread_phase: Option<i64>,
    #[serde(default)]
    typeset_phase: Option<i64>,
    #[serde(default)]
    review_phase: Option<i64>,
    #[serde(default)]
    publish_phase: Option<i64>,
    #[serde(default = "default_offset")]
    offset: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}

pub struct ListWorksetComicsTool {
    prks_client: Arc<PrksClient>,
}

impl ListWorksetComicsTool {
    pub fn new(prks_client: Arc<PrksClient>) -> Self {
        Self { prks_client }
    }
}

#[async_trait::async_trait]
impl ITool for ListWorksetComicsTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "workset_id",
                    PropDef::String {
                        desc: "Target workset id".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "fuzzy_title",
                    PropDef::String {
                        desc: "Optional fuzzy title keyword".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "upload_phase",
                    PropDef::Number {
                        desc: "Optional workflow phase filter, int".to_string(),
                        r#enum: workflow_phase_enum(),
                    },
                ),
                (
                    "translate_phase",
                    PropDef::Number {
                        desc: "Optional workflow phase filter, int".to_string(),
                        r#enum: workflow_phase_enum(),
                    },
                ),
                (
                    "proofread_phase",
                    PropDef::Number {
                        desc: "Optional workflow phase filter, int".to_string(),
                        r#enum: workflow_phase_enum(),
                    },
                ),
                (
                    "typeset_phase",
                    PropDef::Number {
                        desc: "Optional workflow phase filter, int".to_string(),
                        r#enum: workflow_phase_enum(),
                    },
                ),
                (
                    "review_phase",
                    PropDef::Number {
                        desc: "Optional workflow phase filter, int".to_string(),
                        r#enum: workflow_phase_enum(),
                    },
                ),
                (
                    "publish_phase",
                    PropDef::Number {
                        desc: "Optional workflow phase filter, int".to_string(),
                        r#enum: workflow_phase_enum(),
                    },
                ),
                (
                    "offset",
                    PropDef::Number {
                        desc: "Pagination offset, default 0".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "limit",
                    PropDef::Number {
                        desc: "Pagination limit, default 20".to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec!["workset_id".to_string()]);

        ToolDefination::new(
            "list_workset_comics",
            "List comics under one workset. includes is hardcoded.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let args: WorksetComicsArgs = parse_json_args(args)?;
        let data = self
            .prks_client
            .list_workset_comics(
                &args.workset_id,
                args.fuzzy_title.as_deref(),
                args.upload_phase,
                args.translate_phase,
                args.proofread_phase,
                args.typeset_phase,
                args.review_phase,
                args.publish_phase,
                args.offset,
                args.limit,
            )
            .await
            .map_err(ExecutionError::exec_fail)?;

        pretty_json(&data)
    }
}

#[derive(Deserialize)]
struct ComicPinnedArgs {
    comic_id: String,
}

pub struct GetComicPinnedChapterTool {
    prks_client: Arc<PrksClient>,
}

impl GetComicPinnedChapterTool {
    pub fn new(prks_client: Arc<PrksClient>) -> Self {
        Self { prks_client }
    }
}

#[async_trait::async_trait]
impl ITool for GetComicPinnedChapterTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![(
                "comic_id",
                PropDef::String {
                    desc: "Target comic id".to_string(),
                    r#enum: None,
                },
            )])
            .with_required(vec!["comic_id".to_string()]);

        ToolDefination::new(
            "get_comic_pinned_chapter",
            "Get pinned chapter for one comic via /chapters/pinned.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let args: ComicPinnedArgs = parse_json_args(args)?;
        let data = self
            .prks_client
            .get_comic_pinned_chapter(&args.comic_id)
            .await
            .map_err(ExecutionError::exec_fail)?;

        pretty_json(&data)
    }
}

#[derive(Deserialize)]
struct ComicChaptersArgs {
    comic_id: String,
    #[serde(default = "default_offset")]
    offset: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}

pub struct ListComicChaptersTool {
    prks_client: Arc<PrksClient>,
}

impl ListComicChaptersTool {
    pub fn new(prks_client: Arc<PrksClient>) -> Self {
        Self { prks_client }
    }
}

#[async_trait::async_trait]
impl ITool for ListComicChaptersTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "comic_id",
                    PropDef::String {
                        desc: "Target comic id".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "offset",
                    PropDef::Number {
                        desc: "Pagination offset, default 0".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "limit",
                    PropDef::Number {
                        desc: "Pagination limit, default 20".to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec!["comic_id".to_string()]);

        ToolDefination::new(
            "list_comic_chapters",
            "List chapters under one comic. includes is hardcoded.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let args: ComicChaptersArgs = parse_json_args(args)?;
        let data = self
            .prks_client
            .list_comic_chapters(&args.comic_id, args.offset, args.limit)
            .await
            .map_err(ExecutionError::exec_fail)?;

        pretty_json(&data)
    }
}

#[derive(Deserialize)]
struct ChapterAssignmentsArgs {
    chapter_id: String,
    #[serde(default = "default_offset")]
    offset: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}

pub struct ListChapterAssignmentsTool {
    prks_client: Arc<PrksClient>,
}

impl ListChapterAssignmentsTool {
    pub fn new(prks_client: Arc<PrksClient>) -> Self {
        Self { prks_client }
    }
}

#[async_trait::async_trait]
impl ITool for ListChapterAssignmentsTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "chapter_id",
                    PropDef::String {
                        desc: "Target chapter id".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "offset",
                    PropDef::Number {
                        desc: "Pagination offset, default 0".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "limit",
                    PropDef::Number {
                        desc: "Pagination limit, default 20".to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec!["chapter_id".to_string()]);

        ToolDefination::new(
            "list_chapter_assignments",
            "List assignments under one chapter. includes is hardcoded.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let args: ChapterAssignmentsArgs = parse_json_args(args)?;
        let data = self
            .prks_client
            .list_chapter_assignments(&args.chapter_id, args.offset, args.limit)
            .await
            .map_err(ExecutionError::exec_fail)?;

        pretty_json(&data)
    }
}

#[derive(Deserialize)]
struct UserAssignmentsArgs {
    user_id: String,
    #[serde(default = "default_offset")]
    offset: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}

pub struct ListUserAssignmentsTool {
    prks_client: Arc<PrksClient>,
}

impl ListUserAssignmentsTool {
    pub fn new(prks_client: Arc<PrksClient>) -> Self {
        Self { prks_client }
    }
}

#[async_trait::async_trait]
impl ITool for ListUserAssignmentsTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "user_id",
                    PropDef::String {
                        desc: "Target user id".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "offset",
                    PropDef::Number {
                        desc: "Pagination offset, default 0".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "limit",
                    PropDef::Number {
                        desc: "Pagination limit, default 20".to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec!["user_id".to_string()]);

        ToolDefination::new(
            "list_user_assignments",
            "List assignments of one user. includes is hardcoded.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let args: UserAssignmentsArgs = parse_json_args(args)?;
        let data = self
            .prks_client
            .list_user_assignments(&args.user_id, args.offset, args.limit)
            .await
            .map_err(ExecutionError::exec_fail)?;

        pretty_json(&data)
    }
}
