//! Shared HTTP client provider.
//!
//! One configured `reqwest::Client` for the whole app: consistent
//! User-Agent across every outbound call to Resend. When multi-account
//! lands, per-account default headers would plug in here.

use reqwest::Client;

pub fn client() -> Client {
    Client::builder()
        .user_agent(concat!("OnlySend/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("failed to build http client")
}
