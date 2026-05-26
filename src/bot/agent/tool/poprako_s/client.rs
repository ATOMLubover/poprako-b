use serde::Deserialize;
use serde::Serialize;

use crate::bot::agent::value_object::{Assignment, Chapter, Comic, Member, Workset};
use crate::http::HttpClient;
use crate::http::result::HttpError;

pub struct PrksClient {
    auth_token: String,
    http_client: HttpClient,
}

impl PrksClient {
    pub fn new(http_client: HttpClient, auth_token: String) -> Self {
        Self {
            auth_token,
            http_client,
        }
    }

    pub async fn login(
        http_client: &HttpClient,
        qid: &str,
        password: &str,
    ) -> Result<String, String> {
        let payload = LoginArgs { qid, password };
        let envelope: HttpRes<LoginRes> = http_client
            .post("auth/login", &payload, &[], None)
            .await
            .map_err(Self::map_http_error)?;

        match envelope.data {
            Some(data) => Ok(data.token),
            None => Err(envelope
                .message
                .unwrap_or_else(|| format!("login failed with code {}", envelope.code))),
        }
    }

    pub async fn list_my_members(&self, offset: i64, limit: i64) -> Result<Vec<Member>, String> {
        let query = vec![
            ("includes".to_string(), "team".to_string()),
            ("offset".to_string(), offset.to_string()),
            ("limit".to_string(), limit.to_string()),
        ];

        self.get_enveloped("members/mine", &query).await
    }

    pub async fn list_team_worksets(
        &self,
        team_id: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Workset>, String> {
        let query = vec![
            ("team_id".to_string(), team_id.to_string()),
            ("offset".to_string(), offset.to_string()),
            ("limit".to_string(), limit.to_string()),
        ];

        self.get_enveloped("worksets", &query).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_workset_comics(
        &self,
        workset_id: &str,
        fuzzy_title: Option<&str>,
        upload_phase: Option<i64>,
        translate_phase: Option<i64>,
        proofread_phase: Option<i64>,
        typeset_phase: Option<i64>,
        review_phase: Option<i64>,
        publish_phase: Option<i64>,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Comic>, String> {
        let mut query = vec![
            ("workset_id".to_string(), workset_id.to_string()),
            ("offset".to_string(), offset.to_string()),
            ("limit".to_string(), limit.to_string()),
            ("includes".to_string(), "workset".to_string()),
            ("includes".to_string(), "workset.team".to_string()),
            ("includes".to_string(), "creator".to_string()),
        ];

        if let Some(v) = fuzzy_title {
            query.push(("fuzzy_title".to_string(), v.to_string()));
        }
        if let Some(v) = upload_phase {
            query.push(("upload_phase".to_string(), v.to_string()));
        }
        if let Some(v) = translate_phase {
            query.push(("translate_phase".to_string(), v.to_string()));
        }
        if let Some(v) = proofread_phase {
            query.push(("proofread_phase".to_string(), v.to_string()));
        }
        if let Some(v) = typeset_phase {
            query.push(("typeset_phase".to_string(), v.to_string()));
        }
        if let Some(v) = review_phase {
            query.push(("review_phase".to_string(), v.to_string()));
        }
        if let Some(v) = publish_phase {
            query.push(("publish_phase".to_string(), v.to_string()));
        }

        self.get_enveloped("comics", &query).await
    }

    pub async fn get_comic_pinned_chapter(&self, comic_id: &str) -> Result<Chapter, String> {
        let query = vec![("comic_id".to_string(), comic_id.to_string())];
        self.get_enveloped("chapters/pinned", &query).await
    }

    pub async fn list_comic_chapters(
        &self,
        comic_id: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Chapter>, String> {
        let query = vec![
            ("comic_id".to_string(), comic_id.to_string()),
            ("offset".to_string(), offset.to_string()),
            ("limit".to_string(), limit.to_string()),
            ("includes".to_string(), "comic".to_string()),
            ("includes".to_string(), "comic.workset".to_string()),
            ("includes".to_string(), "comic.workset.team".to_string()),
            ("includes".to_string(), "comic.creator".to_string()),
            ("includes".to_string(), "creator".to_string()),
        ];

        self.get_enveloped("chapters", &query).await
    }

    pub async fn list_chapter_assignments(
        &self,
        chapter_id: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Assignment>, String> {
        let query = vec![
            ("chapter_id".to_string(), chapter_id.to_string()),
            ("offset".to_string(), offset.to_string()),
            ("limit".to_string(), limit.to_string()),
            ("includes".to_string(), "user".to_string()),
            ("includes".to_string(), "chapter".to_string()),
            ("includes".to_string(), "chapter.comic".to_string()),
            ("includes".to_string(), "chapter.comic.workset".to_string()),
            (
                "includes".to_string(),
                "chapter.comic.workset.team".to_string(),
            ),
            ("includes".to_string(), "chapter.creator".to_string()),
            ("includes".to_string(), "chapter.comic.creator".to_string()),
        ];

        self.get_enveloped("assignments", &query).await
    }

    pub async fn list_user_assignments(
        &self,
        user_id: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Assignment>, String> {
        let query = vec![
            ("offset".to_string(), offset.to_string()),
            ("limit".to_string(), limit.to_string()),
            ("includes".to_string(), "user".to_string()),
            ("includes".to_string(), "chapter".to_string()),
            ("includes".to_string(), "chapter.comic".to_string()),
            ("includes".to_string(), "chapter.comic.workset".to_string()),
            (
                "includes".to_string(),
                "chapter.comic.workset.team".to_string(),
            ),
            ("includes".to_string(), "chapter.creator".to_string()),
            ("includes".to_string(), "chapter.comic.creator".to_string()),
        ];

        self.get_enveloped(&format!("assignments/users/{user_id}"), &query)
            .await
    }

    async fn get_enveloped<T>(&self, path: &str, query: &[(String, String)]) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        let envelope: HttpRes<T> = self
            .http_client
            .get_with_query(path, query, Some(&self.auth_token))
            .await
            .map_err(Self::map_http_error)?;

        match envelope.data {
            Some(data) => Ok(data),
            None => Err(envelope
                .message
                .unwrap_or_else(|| format!("request failed with code {}", envelope.code))),
        }
    }

    fn map_http_error(err: HttpError) -> String {
        match err {
            HttpError::InvalidUrl(msg)
            | HttpError::ResponseBody(msg)
            | HttpError::Decode(msg)
            | HttpError::Unknown(msg) => msg,
            HttpError::Timeout => "request timeout".to_string(),
        }
    }
}

#[derive(Serialize)]
struct LoginArgs<'a> {
    qid: &'a str,
    password: &'a str,
}

#[derive(Debug, Deserialize)]
struct LoginRes {
    token: String,
}

#[derive(Debug, Deserialize)]
struct HttpRes<T> {
    code: i32,
    message: Option<String>,
    data: Option<T>,
}
