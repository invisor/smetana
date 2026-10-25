//! The Tauri wrappers over the attachment store in `smetana_core::attachments`.
//!
//! The one thing they add is the tracker worker's handle. It is looked up here, as an `Option`, and
//! handed in: the store asks the worker which project is open, and an app whose worker is not
//! running is an answer the store has a message for rather than a crash.

use tauri::{AppHandle, Manager};

use smetana_core::attachments::{Attachment, AttachmentError, Cleaned, Survey};
use crate::host;
use smetana_core::tracker::service::TrackerHandle;

#[tauri::command]
pub async fn attachments_survey(app: AppHandle) -> Result<Survey, AttachmentError> {
    let tracker = app.try_state::<TrackerHandle>();
    smetana_core::attachments::survey_store(&host::context(&app), tracker.as_deref()).await
}

#[tauri::command]
pub async fn attachments_clean(app: AppHandle) -> Result<Cleaned, AttachmentError> {
    let tracker = app.try_state::<TrackerHandle>();
    smetana_core::attachments::clean_store(&host::context(&app), tracker.as_deref()).await
}

#[tauri::command]
pub async fn attachment_import(app: AppHandle, path: String) -> Result<Attachment, AttachmentError> {
    let tracker = app.try_state::<TrackerHandle>();
    smetana_core::attachments::import_file(&host::context(&app), tracker.as_deref(), path).await
}

#[tauri::command]
pub async fn attachment_reopen(app: AppHandle, path: String) -> Result<Attachment, AttachmentError> {
    smetana_core::attachments::reopen_stored(&host::context(&app), path).await
}

#[tauri::command]
pub async fn attachment_write(
    app: AppHandle,
    name: Option<String>,
    data: String,
) -> Result<Attachment, AttachmentError> {
    let tracker = app.try_state::<TrackerHandle>();
    smetana_core::attachments::write_pasted(&host::context(&app), tracker.as_deref(), name, data).await
}

/// A file an agent wrote into its own prose, read by path rather than out of the store.
#[tauri::command]
pub async fn image_read(base: String, src: String) -> Result<Attachment, AttachmentError> {
    smetana_core::attachments::figure::image_read(base, src).await
}
