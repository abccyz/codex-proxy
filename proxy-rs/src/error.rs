//! 统一错误类型 — 使用 thiserror 替代裸 String 传递

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// hub 代理服务的统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("无效的 JSON 请求: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("上游请求失败: {0}")]
    UpstreamError(String),

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("上游返回无效响应: {0}")]
    InvalidUpstreamResponse(String),

    #[error("代理内部错误: {0}")]
    Internal(String),

    #[error("上游 HTTP {0}: {1}")]
    UpstreamHttp(u16, String),

    #[error("请求错误: {0}")]
    Request(#[from] reqwest::Error),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self {
            ProxyError::InvalidJson(_) => (StatusCode::BAD_REQUEST, "invalid_request"),
            ProxyError::ConfigError(_) => (StatusCode::BAD_GATEWAY, "config_error"),
            ProxyError::UpstreamError(_) | ProxyError::UpstreamHttp(_, _) => {
                (StatusCode::BAD_GATEWAY, "upstream_error")
            }
            ProxyError::InvalidUpstreamResponse(_) | ProxyError::Internal(_) => {
                (StatusCode::BAD_GATEWAY, "proxy_error")
            }
            ProxyError::Request(_) => (StatusCode::BAD_GATEWAY, "proxy_error"),
        };

        let body = serde_json::json!({
            "error": {
                "message": self.to_string(),
                "type": error_type,
            }
        });

        (status, [(axum::http::header::CONTENT_TYPE, "application/json")], body.to_string())
            .into_response()
    }
}
