use std::path::{Path, PathBuf};

/// Resolve a repository asset without coupling game code to the process working directory.
pub fn asset_path(relative_path: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("assets")
        .join(relative_path)
}

#[cfg(test)]
mod tests {
    use super::asset_path;

    #[test]
    fn resolves_from_the_shared_asset_tree() {
        assert!(asset_path("models/farm/scene.gltf").ends_with("assets/models/farm/scene.gltf"));
    }
}
