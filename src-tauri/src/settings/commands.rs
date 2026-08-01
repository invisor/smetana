//! Команды настроек: тонкие, без собственного состояния.
//!
//! В отличие от трекера, воркер здесь не нужен: истина по настройкам живёт во
//! фронте, снаружи их никто не меняет, а файл читается и пишется за
//! миллисекунды — очередь запросов сторожила бы то, за что никто не борется.
//!
//! Активный проект приходит от фронта и лежит в самом файле. Раньше он
//! добывался у трекера, и это была подпорка: настройки зависели от трекера
//! ради значения, которое им же и принадлежит.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use super::file;
use super::model::{merge, resolve, ResolvedSettings};
use crate::project;

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

/// `project` — «покажи состояние вот этого проекта»: так фронт получает
/// раскладку другого проекта при переключении. Без аргумента отвечаем про
/// активный из файла.
#[tauri::command]
pub async fn settings_load(
    app: AppHandle,
    project: Option<String>,
) -> Result<ResolvedSettings, SettingsError> {
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

    let mut view = resolve(&settings, project.as_deref());
    // Первый запуск: списка ещё нет. Открываемся тем каталогом, из которого
    // запустили, если он отслеживается; в список его положит фронт — файл
    // чтением не меняется.
    if view.open_projects.is_empty() && view.active_project.is_none() {
        view.active_project =
            project::default_project().map(|dir| dir.to_string_lossy().into_owned());
    }
    Ok(view)
}

/// Файл перечитывается на каждую запись: между двумя сохранениями его мог
/// поправить человек, и записи других проектов в нём терять нельзя.
#[tauri::command]
pub async fn settings_save(app: AppHandle, settings: ResolvedSettings) -> Result<(), SettingsError> {
    let path = settings_path(&app)?;
    let (mut stored, problem) = file::load(&path);

    /* Асимметрия намеренная. Сломанный и слишком новый файл уже уехали в
       .bak — там записывать поверх безопасно, а отказ оставил бы человека
       навсегда без возможности сохраниться. С нечитаемым файлом наоборот:
       копию снять было нечем, а `rename` спрашивает права только у каталога,
       так что запись молча стёрла бы то, что мы даже не смогли прочесть. */
    if matches!(problem, Some(file::Problem::Unreadable)) {
        return Err(SettingsError::Write(format!(
            "{}: существующий файл не удалось прочитать, поэтому он не был перезаписан",
            path.display()
        )));
    }

    merge(&mut stored, settings, chrono::Utc::now().to_rfc3339());
    file::save(&path, &stored).map_err(SettingsError::Write)
}
