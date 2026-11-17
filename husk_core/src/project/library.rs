//! Husk's own project folder: the projects the Home screen lists. Husk keeps at
//! most `MAX_PROJECTS` there. Files saved anywhere else belong to the user and
//! are never counted, listed or touched.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The most projects Husk keeps in its own folder at once.
pub const MAX_PROJECTS: usize = 2;

const EXTENSION: &str = "husk";

/// One project in Husk's folder, as the Home screen shows it.
pub struct Entry {
    pub name: String,
    pub path: String,
    pub seconds_ago: u64,
    pub bytes: u64,
}

fn is_project_file(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some(EXTENSION)
}

/// Where a project's thumbnail lives: beside it, with the extension `thumb`.
pub fn thumbnail_path(project: &Path) -> PathBuf {
    project.with_extension("thumb")
}

/// The projects in `dir`, newest first.
pub fn list_in(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let now = SystemTime::now();
    let mut found: Vec<(SystemTime, Entry)> = Vec::new();
    for item in read.flatten() {
        let path = item.path();
        if !is_project_file(&path) {
            continue;
        }
        let Ok(meta) = item.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let modified = meta.modified().unwrap_or(now);
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Untitled")
            .to_string();
        found.push((
            modified,
            Entry {
                name,
                path: path.to_string_lossy().to_string(),
                seconds_ago: now
                    .duration_since(modified)
                    .map(|elapsed| elapsed.as_secs())
                    .unwrap_or(0),
                bytes: meta.len(),
            },
        ));
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|(_, entry)| entry).collect()
}

/// A path in `dir` for a new project called `base`: "Untitled", then
/// "Untitled 2", "Untitled 3" and so on, never a name already in use.
pub fn new_path_in(dir: &Path, base: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{base}.{EXTENSION}"));
    let mut number = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{base} {number}.{EXTENSION}"));
        number += 1;
    }
    candidate
}

/// Deletes the project at `path` and its thumbnail, but only if it is a
/// project file directly inside `dir`.
pub fn delete_in(dir: &Path, path: &Path) -> Result<(), String> {
    if path.parent() != Some(dir) || !is_project_file(path) {
        return Err("Husk only deletes projects in its own folder".to_string());
    }
    std::fs::remove_file(path)
        .map_err(|e| format!("Could not delete {}: {e}", path.display()))?;
    let _ = std::fs::remove_file(thumbnail_path(path));
    Ok(())
}

/// Shrinks RGBA pixels (`width` x `height`) to about `target_width` wide by
/// averaging whole blocks of pixels. Returns the new width, height and pixels,
/// or None when the pixels do not match the size.
pub fn shrink(
    pixels: &[u8],
    width: usize,
    height: usize,
    target_width: usize,
) -> Option<(usize, usize, Vec<u8>)> {
    if width == 0 || height == 0 || pixels.len() != width * height * 4 {
        return None;
    }
    let factor = (width / target_width.max(1)).max(1);
    let out_width = width / factor;
    let out_height = height / factor;
    if out_width == 0 || out_height == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(out_width * out_height * 4);
    let count = (factor * factor) as u32;
    for out_y in 0..out_height {
        for out_x in 0..out_width {
            let mut sum = [0u32; 4];
            for dy in 0..factor {
                for dx in 0..factor {
                    let at = ((out_y * factor + dy) * width + out_x * factor + dx) * 4;
                    for channel in 0..4 {
                        sum[channel] += pixels[at + channel] as u32;
                    }
                }
            }
            for channel in 0..4 {
                out.push((sum[channel] / count) as u8);
            }
        }
    }
    Some((out_width, out_height, out))
}

/// Saves a project's thumbnail: width and height (u32 each), then the RGBA
/// pixels. A thumbnail that cannot be written is simply missing.
pub fn write_thumbnail(project: &Path, width: usize, height: usize, rgba: &[u8]) {
    let mut bytes = Vec::with_capacity(8 + rgba.len());
    bytes.extend_from_slice(&(width as u32).to_le_bytes());
    bytes.extend_from_slice(&(height as u32).to_le_bytes());
    bytes.extend_from_slice(rgba);
    let _ = std::fs::write(thumbnail_path(project), bytes);
}

/// A project's thumbnail as (width, height, RGBA pixels), if it has a good one.
pub fn read_thumbnail(project: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = std::fs::read(thumbnail_path(project)).ok()?;
    if bytes.len() < 8 {
        return None;
    }
    let width = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let pixels = bytes[8..].to_vec();
    if pixels.len() as u64 != width as u64 * height as u64 * 4 {
        return None;
    }
    Some((width, height, pixels))
}

/// Husk's own project folder.
pub fn projects_dir() -> Option<PathBuf> {
    Some(super::recovery::app_data_dir()?.join("Projects"))
}

/// The projects in Husk's own folder, newest first.
pub fn list() -> Vec<Entry> {
    projects_dir()
        .map(|dir| list_in(&dir))
        .unwrap_or_default()
}

/// A fresh path for a new project in Husk's folder (made if missing), or None
/// when the folder already holds `MAX_PROJECTS` or cannot be made.
pub fn new_path() -> Option<PathBuf> {
    let dir = projects_dir()?;
    if list_in(&dir).len() >= MAX_PROJECTS {
        return None;
    }
    std::fs::create_dir_all(&dir).ok()?;
    Some(new_path_in(&dir, "Untitled"))
}

/// Deletes a project from Husk's own folder.
pub fn delete(path: &str) -> Result<(), String> {
    let dir = projects_dir()
        .ok_or_else(|| "Husk could not find its project folder".to_string())?;
    delete_in(&dir, Path::new(path))
}

/// Whether `path` is a project file directly inside Husk's own folder.
pub fn is_managed(path: &str) -> bool {
    match (projects_dir(), Path::new(path).parent()) {
        (Some(dir), Some(parent)) => parent == dir && is_project_file(Path::new(path)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("husk_library_test_{}_{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Makes a small file that looks `age_seconds` old.
    fn touch(dir: &Path, name: &str, age_seconds: u64) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"x").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - std::time::Duration::from_secs(age_seconds))
            .unwrap();
        path
    }

    #[test]
    fn the_projects_in_a_folder_are_listed_newest_first_and_other_files_are_ignored() {
        let dir = temp_dir("list");
        touch(&dir, "old.husk", 5000);
        touch(&dir, "new.husk", 10);
        touch(&dir, "notes.txt", 1);
        touch(&dir, "new.thumb", 1);
        let entries = list_in(&dir);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "new");
        assert_eq!(entries[1].name, "old");
        assert!(entries[1].seconds_ago >= 4990);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_project_never_takes_a_name_that_is_in_use() {
        let dir = temp_dir("names");
        assert_eq!(new_path_in(&dir, "Untitled"), dir.join("Untitled.husk"));
        touch(&dir, "Untitled.husk", 1);
        assert_eq!(new_path_in(&dir, "Untitled"), dir.join("Untitled 2.husk"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_projects_inside_the_folder_can_be_deleted_and_the_thumbnail_goes_with_them() {
        let dir = temp_dir("delete");
        let other = temp_dir("delete_other");
        let inside = touch(&dir, "a.husk", 1);
        write_thumbnail(&inside, 1, 1, &[1, 2, 3, 4]);
        let outside = touch(&other, "b.husk", 1);

        assert!(delete_in(&dir, &outside).is_err());
        assert!(outside.exists());
        assert!(delete_in(&dir, &inside).is_ok());
        assert!(!inside.exists());
        assert!(!thumbnail_path(&inside).exists());
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&other).ok();
    }

    #[test]
    fn a_frame_shrinks_by_averaging_blocks_and_a_thumbnail_reads_back() {
        // A 4 x 2 frame: the left 2 x 2 block black, the right one grey.
        let mut pixels = Vec::new();
        for _ in 0..2 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 200 };
                pixels.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let (width, height, small) = shrink(&pixels, 4, 2, 2).unwrap();
        assert_eq!((width, height), (2, 1));
        assert_eq!(&small[0..4], &[0, 0, 0, 255]);
        assert_eq!(&small[4..8], &[200, 200, 200, 255]);
        assert!(shrink(&pixels[..7], 4, 2, 2).is_none());

        let dir = temp_dir("thumb");
        let project = dir.join("p.husk");
        write_thumbnail(&project, width, height, &small);
        let (read_width, read_height, read_pixels) = read_thumbnail(&project).unwrap();
        assert_eq!((read_width, read_height), (2, 1));
        assert_eq!(read_pixels, small);
        std::fs::remove_dir_all(&dir).ok();
    }
}