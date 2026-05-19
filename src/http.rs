// HTTP client and server implementations for app.

use reqwest::Client;
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
        let url = self.base_url.join(path)?;
        let response = self.reqwest.get(url).send().await?;

        Ok(response.json().await?)
    }

    pub async fn post<P, R>(&self, path: &str, payload: &P) -> HttpResult<R>
    where
        P: serde::ser::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let url = self.base_url.join(path)?;
        let response = self.reqwest.post(url).json(payload).send().await?;

        Ok(response.json().await?)
    }
}

pub mod result {
    use url::ParseError;

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
