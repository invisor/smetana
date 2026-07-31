//! Команды настроек: тонкие, без собственного состояния.
//!
//! В отличие от трекера, воркер здесь не нужен: истина по настройкам живёт во
//! фронте, снаружи их никто не меняет, а файл читается и пишется за
//! миллисекунды — очередь запросов сторожила бы то, за что никто не борется.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use super::file;
use super::model::{merge, resolve, ResolvedSettings};
use crate::tracker::service::project_dir;

/// Ошибок чтения у настроек почти нет: файла может не быть, он может быть
/// сломан — это обычная жизнь, а не отказ. Настоящих бед две: некуда писать
/// и не получилось записать.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("не удалось определить каталог настроек: {0}")]
    Dir(String),
    #[error("не удалось сохранить настройки: {0}")]
    Write(String),
}

// Tauri требует, чтобы ошибка команды умела сериализоваться.
impl serde::Serialize for SettingsError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, SettingsError> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("settings.json"))
        .map_err(|err| SettingsError::Dir(err.to_string()))
}

/// Текущий проект — тот же каталог, который смотрит трекер. Функция чистая и
/// в пределах запуска не меняется, поэтому её просто зовут, а не хранят.
fn current_project() -> String {
    project_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn settings_load(app: AppHandle) -> Result<ResolvedSettings, SettingsError> {
    let path = settings_path(&app)?;
    let (settings, problem) = file::load(&path);
    match problem {
        Some(file::Problem::Broken) => {
            log::warn!("настройки: {} не прочитался, копия рядом в .bak", path.display())
        }
        Some(file::Problem::TooNew) => {
            log::warn!("настройки: {} новее этой сборки, копия рядом в .bak", path.display())
        }
        Some(file::Problem::Unreadable) => {
            log::warn!("настройки: {} не удалось прочитать, настройки начались с умолчаний", path.display())
        }
        None => {}
    }
    Ok(resolve(&settings, &current_project()))
}

/// Файл перечитывается на каждую запись: между двумя сохранениями его мог
/// поправить человек, и записи других проектов в нём терять нельзя.
#[tauri::command]
pub async fn settings_save(app: AppHandle, settings: ResolvedSettings) -> Result<(), SettingsError> {
    let path = settings_path(&app)?;
    let (mut stored, _) = file::load(&path);
    merge(&mut stored, settings, &current_project(), chrono::Utc::now().to_rfc3339());
    file::save(&path, &stored).map_err(SettingsError::Write)
}
