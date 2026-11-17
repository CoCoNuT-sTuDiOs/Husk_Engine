//! The recovery copy: a project file Husk writes by itself every couple of
//! minutes, so a crash or a power cut does not lose the scene.

use std::path::{Path, PathBuf};

use crate::scene::Scene;

const FILE_NAME: &str = "recovery.husk";

/// The folder Husk keeps its own files in, if the system says where.
pub fn app_data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join("Husk"))
}

fn recovery_file(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// Writes the recovery copy of `scene` into `dir`, making the folder if needed.
pub fn write_recovery_in(dir: &Path, scene: &Scene) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let path = recovery_file(dir);
    super::scene_file::write_scene(&path.to_string_lossy(), scene).map(|_| ())
}

/// The recovery copy's path and how many seconds old it is, if there is one.
pub fn recovery_info_in(dir: &Path) -> Option<(String, u64)> {
    let path = recovery_file(dir);
    let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
    let age = std::time::SystemTime::now()
        .duration_since(modified)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    Some((path.to_string_lossy().to_string(), age))
}

/// Deletes the recovery copy in `dir`, if there is one.
pub fn clear_recovery_in(dir: &Path) {
    let _ = std::fs::remove_file(recovery_file(dir));
}

/// Writes the recovery copy into Husk's own folder.
pub fn write_recovery(scene: &Scene) -> Result<(), String> {
    let dir = app_data_dir()
        .ok_or_else(|| "Husk could not find a folder for its own files".to_string())?;
    write_recovery_in(&dir, scene)
}

pub fn recovery_info() -> Option<(String, u64)> {
    recovery_info_in(&app_data_dir()?)
}

pub fn clear_recovery() {
    if let Some(dir) = app_data_dir() {
        clear_recovery_in(&dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recovery_copy_is_written_found_read_back_and_cleared() {
        let dir = std::env::temp_dir().join(format!("husk_recovery_test_{}", std::process::id()));
        assert!(recovery_info_in(&dir).is_none());

        let mut scene = Scene::default();
        scene.push_object(crate::scene::Object::default());
        write_recovery_in(&dir, &scene).unwrap();

        let (path, age) = recovery_info_in(&dir).expect("the copy was written");
        assert!(age < 60);
        let reopened = super::super::scene_file::read_scene(&path).unwrap();
        assert_eq!(reopened.objects.len(), 1);

        clear_recovery_in(&dir);
        assert!(recovery_info_in(&dir).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}