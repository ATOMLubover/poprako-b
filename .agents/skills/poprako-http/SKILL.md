---
name: poprako-http
description: |
  Internal skill for the poprako-b-preview HTTP layer. Covers HttpClient and
  HttpError types in src/http.rs. Use when working on direct HTTP calls
  (currently unused by the resolver which delegates to openai-oxide).
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
    pub fn new(base_url: Url) -> Self;

    pub async fn get<R: DeserializeOwned>(&self, path: &str) -> HttpResult<R>;
    pub async fn post<P: Serialize, R: DeserializeOwned>(&self, path: &str, payload: &P) -> HttpResult<R>;
}
```

`path` is joined against `base_url` via `Url::join`. Responses are deserialized from JSON.

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

### `HttpResult<T>`

`type HttpResult<T> = Result<T, HttpError>;`

## Status

This layer is **not currently used** by the resolver. The `OpenAiResolver` delegates all HTTP to `openai_oxide::OpenAI`. The `HttpClient` will be used once the application grows its own REST endpoints.
