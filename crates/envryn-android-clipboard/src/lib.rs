#![cfg(target_os = "android")]

//! Android-only clipboard bridge for secret material.
//!
//! Tauri's webview clipboard cannot mark a clip as sensitive. This bridge
//! keeps secret-copying behind Envryn's existing Rust IPC command and adds
//! Android's `android.content.extra.IS_SENSITIVE` metadata before the value
//! reaches the system clipboard.

use serde::de::DeserializeOwned;
use tauri::{
    plugin::{Builder, PluginApi, PluginHandle, TauriPlugin},
    AppHandle, Manager, Runtime,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteRequest {
    text: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadResponse {
    text: String,
}

pub struct SensitiveClipboard<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> SensitiveClipboard<R> {
    pub fn write_sensitive_text(&self, text: String) -> Result<()> {
        self.0
            .run_mobile_plugin("writeSensitiveText", WriteRequest { text })
            .map_err(Into::into)
    }

    pub fn read_text(&self) -> Result<String> {
        self.0
            .run_mobile_plugin::<ReadResponse>("readText", ())
            .map(|response| response.text)
            .map_err(Into::into)
    }

    pub fn clear(&self) -> Result<()> {
        self.0.run_mobile_plugin("clear", ()).map_err(Into::into)
    }
}

pub trait SensitiveClipboardExt<R: Runtime> {
    fn sensitive_clipboard(&self) -> &SensitiveClipboard<R>;
}

impl<R: Runtime, T: Manager<R>> SensitiveClipboardExt<R> for T {
    fn sensitive_clipboard(&self) -> &SensitiveClipboard<R> {
        self.state::<SensitiveClipboard<R>>().inner()
    }
}

fn init_mobile<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<SensitiveClipboard<R>> {
    let handle = api.register_android_plugin("dev.envryn.clipboard", "SensitiveClipboardPlugin")?;
    Ok(SensitiveClipboard(handle))
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("envryn-sensitive-clipboard")
        .setup(|app, api| {
            let clipboard = init_mobile(app, api)?;
            app.manage(clipboard);
            Ok(())
        })
        .build()
}
