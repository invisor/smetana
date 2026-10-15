//! The two outbound sides. The gateway talks to them through one narrow trait so that a test can
//! stand in for Apple and Google and look at exactly what would have been sent.

pub mod apns;
pub mod fcm;

use std::future::Future;
use std::pin::Pin;

/// What reaches a provider: the device token and the blob, exactly as the phone's desktop sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushMessage {
    pub token: String,
    /// The base64 text received in the request, forwarded verbatim. Never decoded for use.
    pub blob: String,
}

/// A provider failure, coarse on purpose: a reqwest error prints the request URL, and an APNs URL
/// carries the device token, so nothing from the underlying error is allowed to travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderError {
    /// The provider says this token is no longer valid (APNs 410, FCM UNREGISTERED).
    TokenGone,
    /// Anything else: network, credentials, provider outage.
    Unavailable,
}

pub type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), ProviderError>> + Send + 'a>>;

pub trait Provider: Send + Sync {
    fn send<'a>(&'a self, message: &'a PushMessage) -> SendFuture<'a>;
}
