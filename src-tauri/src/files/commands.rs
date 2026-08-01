//! Команды файлов: тонкие, без собственного состояния — как у настроек.
//!
//! `root` приходит от фронта: он и так знает активный проект, а держать здесь
//! вторую копию этого знания значило бы завести зависимость от трекера ради
//! значения, которое ему не принадлежит. Каждая команда проверяет, что путь
//! лежит внутри присланного корня.

use std::path::PathBuf;

use super::fs;
use super::model::{FileText, FilesError, Listing, Stat};

#[tauri::command]
pub async fn files_list(root: String, dir: String) -> Result<Listing, FilesError> {
    fs::list_dir(&PathBuf::from(root), &dir)
}

#[tauri::command]
pub async fn files_read(root: String, path: String) -> Result<FileText, FilesError> {
    fs::read_text(&PathBuf::from(root), &path)
}

/// Отвечает новой меткой времени: фронт кладёт её в буфер и присылает
/// следующей записью.
#[tauri::command]
pub async fn files_write(
    root: String,
    path: String,
    text: String,
    expected_mtime: i64,
) -> Result<i64, FilesError> {
    fs::write_text(&PathBuf::from(root), &path, &text, expected_mtime)
}

/// Отказов нет: исчезнувший файл приезжает как `mtime: null`.
#[tauri::command]
pub async fn files_stat(root: String, paths: Vec<String>) -> Vec<Stat> {
    fs::stat_many(&PathBuf::from(root), &paths)
}
