---
name: poprako-http
description: |
  Internal skill for the poprako-b-preview HTTP layer. Covers HttpClient and
  HttpError types in src/http.rs. Use when working on direct HTTP calls,
  bearer-token JSON APIs, DeepSeek resolver transport, query parameters, or
  error mapping around reqwest responses.
---

# Poprako HTTP Layer

## File

```
src/http.rs
```

## Types

### `HttpClient`

Thin wrapper around `reqwest::Client` with a pre-configured `base_url`. Provides:

```rust
impl HttpClient {
    pub fn new(base_url: Option<Url>) -> Self;

    pub async fn get<R>(&self, url: Url) -> HttpResult<R>;

    pub async fn get_with_query<R>(
        &self,
        url: Url,
        query: &[(String, String)],
        bearer_token: Option<&str>,
    ) -> HttpResult<R>;

    pub async fn post<P, R>(
        &self,
        url: Url,
        payload: &P,
        query: &[(String, String)],
        bearer_token: Option<&str>,
    ) -> HttpResult<R>;
}
```

If `base_url` is `Some`, request URLs are joined against it via `Url::join`; otherwise the provided `Url` is used as-is. Responses are deserialized from JSON.

`get_with_query` and `post` append query pairs before sending. `post` always sends JSON with `Content-Type: application/json`. Both bearer-aware methods add `Authorization: Bearer <token>` when a token is supplied.

### `HttpError`

```rust
pub enum HttpError {
    InvalidUrl(String),
    Timeout,
    ResponseBody(String),
    Decode(String),
    Unknown(String),
}
```

Conversions from `url::ParseError` and `reqwest::Error` are implemented via `From`:

- `ParseError` → `InvalidUrl`
- `reqwest::Error::is_timeout()` → `Timeout`
- `reqwest::Error::is_body()` → `ResponseBody`
- `reqwest::Error::is_decode()` → `Decode`
- Everything else → `Unknown`

Non-2xx HTTP responses are converted manually to `ResponseBody("HTTP <status>: <body>")`.

### `HttpResult<T>`

`type HttpResult<T> = Result<T, HttpError>;`

## Status

This layer is used by `DeepSeekResolver` (`src/ai/resolver_impl/deepseek.rs`) for direct OpenAI-compatible chat-completions calls. `OpenAiResolver` still delegates transport to `openai-oxide`.
