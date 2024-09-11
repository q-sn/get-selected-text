#![deny(clippy::all)]

use napi::bindgen_prelude::*;

#[macro_use]
extern crate napi_derive;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
use crate::windows::async_get_text;

#[napi]
pub async fn get_text() -> Result<String> {
  async_get_text().await.map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
}