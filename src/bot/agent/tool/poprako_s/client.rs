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
            .post_path("auth/login", &payload, &[], None)
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

        self.get_enveloped(&format!("assignments/users/{}", user_id), &query)
            .await
    }

    async fn get_enveloped<T>(&self, path: &str, query: &[(String, String)]) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        let envelope: HttpRes<T> = self
            .http_client
            .get_path_with_query(path, query, Some(&self.auth_token))
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

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use url::Url;

    use super::PrksClient;
    use crate::http::HttpClient;

    fn spawn_json_server<F>(assert_request: F, status_code: u16, body: String) -> String
    where
        F: Fn(&str) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("read local addr");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept one connection");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .expect("set read timeout");

            let mut req = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        req.extend_from_slice(&buf[..n]);
                        if req.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            let req_text = String::from_utf8_lossy(&req).to_string();
            let header_end = req_text
                .find("\r\n\r\n")
                .expect("request header terminator");
            let headers = &req_text[..header_end];

            let content_length = headers
                .lines()
                .find_map(|line| {
                    let lower = line.to_ascii_lowercase();
                    lower
                        .strip_prefix("content-length: ")
                        .and_then(|v| v.parse::<usize>().ok())
                })
                .unwrap_or(0);

            if content_length > 0 {
                let mut consumed_body = req.len() - (header_end + 4);
                while consumed_body < content_length {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            req.extend_from_slice(&buf[..n]);
                            consumed_body += n;
                        }
                        Err(_) => break,
                    }
                }
            }

            let req_text = String::from_utf8_lossy(&req).to_string();
            assert_request(&req_text);

            let status_text = if status_code == 200 { "OK" } else { "ERR" };
            let resp = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status_code,
                status_text,
                body.len(),
                body
            );

            stream.write_all(resp.as_bytes()).expect("write response");
            stream.flush().expect("flush response");
        });

        format!("http://{}/api/v1/", addr)
    }

    #[tokio::test]
    async fn login_returns_token() {
        let base_url = spawn_json_server(
            |req| {
                assert!(req.starts_with("POST /api/v1/auth/login"));
                assert!(
                    req.contains("\r\ncontent-type: application/json\r\n")
                        || req.contains("\r\nContent-Type: application/json\r\n")
                );
                assert!(req.contains("\"qid\":\"bot-qid\""));
                assert!(req.contains("\"password\":\"bot-pass\""));
            },
            200,
            r#"{"code":200,"data":{"token":"tok-123"}}"#.to_string(),
        );

        let http_client = HttpClient::new(Some(Url::parse(&base_url).expect("valid base url")));
        let token = PrksClient::login(&http_client, "bot-qid", "bot-pass")
            .await
            .expect("login should succeed");

        assert_eq!(token, "tok-123");
    }

    #[tokio::test]
    async fn login_returns_message_on_missing_data() {
        let base_url = spawn_json_server(
            |_req| {},
            200,
            r#"{"code":400,"message":"bad credentials"}"#.to_string(),
        );

        let http_client = HttpClient::new(Some(Url::parse(&base_url).expect("valid base url")));
        let err = PrksClient::login(&http_client, "bot-qid", "bad-pass")
            .await
            .expect_err("login should fail");

        assert_eq!(err, "bad credentials");
    }

    #[tokio::test]
    async fn list_workset_comics_sends_hardcoded_includes_and_filters() {
        let base_url = spawn_json_server(
            |req| {
                let req_lower = req.to_ascii_lowercase();
                assert!(req.starts_with("GET /api/v1/comics?"));
                assert!(req_lower.contains("authorization: bearer auth-tok\r\n"));
                assert!(req.contains("workset_id=ws_1"));
                assert!(req.contains("fuzzy_title=abc"));
                assert!(req.contains("upload_phase=1"));
                assert!(req.contains("translate_phase=2"));
                assert!(req.contains("proofread_phase=0"));
                assert!(req.contains("typeset_phase=1"));
                assert!(req.contains("review_phase=2"));
                assert!(req.contains("publish_phase=0"));
                assert!(req.contains("offset=5"));
                assert!(req.contains("limit=10"));
                assert!(req.contains("includes=workset"));
                assert!(req.contains("includes=workset.team"));
                assert!(req.contains("includes=creator"));
            },
            200,
            r#"{"code":200,"data":[]}"#.to_string(),
        );

        let http_client = HttpClient::new(Some(Url::parse(&base_url).expect("valid base url")));
        let client = PrksClient::new(http_client, "auth-tok".to_string());

        let comics = client
            .list_workset_comics(
                "ws_1",
                Some("abc"),
                Some(1),
                Some(2),
                Some(0),
                Some(1),
                Some(2),
                Some(0),
                5,
                10,
            )
            .await
            .expect("list should succeed");

        assert!(comics.is_empty());
    }
}
