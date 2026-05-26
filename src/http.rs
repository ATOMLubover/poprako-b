// HTTP client and server implementations for app.

use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use url::Url;

use crate::http::result::HttpResult;

pub struct HttpClient {
    reqwest: Client,
    base_url: Url,
}

impl HttpClient {
    pub fn new(base_url: Url) -> Self {
        Self {
            reqwest: reqwest::Client::new(),
            base_url,
        }
    }

    pub async fn get<R>(&self, path: &str) -> HttpResult<R>
    where
        R: serde::de::DeserializeOwned,
    {
        self.get_with_query::<R>(path, &[], None).await
    }

    pub async fn get_with_query<R>(
        &self,
        path: &str,
        query: &[(String, String)],
        bearer_token: Option<&str>,
    ) -> HttpResult<R>
    where
        R: serde::de::DeserializeOwned,
    {
        let mut url = self.base_url.join(path)?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in query {
                pairs.append_pair(k, v);
            }
        }

        let mut req = self.reqwest.get(url);
        if let Some(token) = bearer_token {
            req = req.header(AUTHORIZATION, format!("Bearer {token}"));
        }

        let response = req.send().await?;

        Ok(response.json().await?)
    }

    pub async fn post<P, R>(
        &self,
        path: &str,
        payload: &P,
        query: &[(String, String)],
        bearer_token: Option<&str>,
    ) -> HttpResult<R>
    where
        P: serde::ser::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let mut url = self.base_url.join(path)?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in query {
                pairs.append_pair(k, v);
            }
        }

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(token) = bearer_token {
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| result::HttpError::Unknown(e.to_string()))?;
            headers.insert(AUTHORIZATION, value);
        }

        let response = self
            .reqwest
            .post(url)
            .headers(headers)
            .json(payload)
            .send()
            .await?;

        Ok(response.json().await?)
    }
}

pub mod result {
    use url::ParseError;

    #[derive(Debug)]
    pub enum HttpError {
        InvalidUrl(String),
        Timeout,
        ResponseBody(String),
        Decode(String),
        Unknown(String),
    }

    impl From<ParseError> for HttpError {
        fn from(e: ParseError) -> Self {
            HttpError::InvalidUrl(e.to_string())
        }
    }

    impl From<reqwest::Error> for HttpError {
        fn from(e: reqwest::Error) -> Self {
            if e.is_timeout() {
                HttpError::Timeout
            } else if e.is_body() {
                HttpError::ResponseBody(e.to_string())
            } else if e.is_decode() {
                HttpError::Decode(e.to_string())
            } else {
                HttpError::Unknown(e.to_string())
            }
        }
    }

    pub type HttpResult<T> = std::result::Result<T, HttpError>;
}
