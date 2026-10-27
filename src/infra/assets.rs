//! Content-hash cache busting for everything served from `public/` under
//! `/assets`.
//!
//! Templates link static files through `SharedContext::asset`, which appends
//! `?v=<hash of the file's contents>`. A file's URL therefore changes exactly
//! when its contents do: browsers can cache it forever, a deploy that touches
//! one script re-downloads only that script, and every replica agrees on the
//! URL. `cache_control` sends the long-lived header only for a request whose
//! `v` matches the file as it is now; anything else is cached briefly.

use crate::prelude::*;
use axum::extract::{Request, State};
use axum::http::HeaderValue;
use axum::http::header::CACHE_CONTROL;
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const URL_PREFIX: &str = "/assets/";
/// Hex characters of the SHA-256 kept in the URL — plenty to tell versions
/// of one file apart.
const HASH_LEN: usize = 12;
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// For requests without a current `v` (images built from data, like flags,
/// or a page rendered before a deploy). Short enough that nothing is stuck
/// stale for long.
const SHORT_LIVED: &str = "public, max-age=3600";

#[derive(Clone)]
pub struct AssetManifest {
    root: PathBuf,
    /// Hash of every file under `root`, keyed by its path relative to it.
    /// `None` in debug builds, where files are hashed on every lookup so
    /// edits made while `./dev.sh` runs show up without a restart.
    hashes: Option<Arc<HashMap<String, String>>>,
}

impl AssetManifest {
    pub fn load(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        if cfg!(debug_assertions) {
            return Self { root, hashes: None };
        }
        let mut hashes = HashMap::new();
        collect(&root, &root, &mut hashes);
        info!(files = hashes.len(), "hashed static assets");
        Self {
            root,
            hashes: Some(Arc::new(hashes)),
        }
    }

    /// `path` with `?v=<content hash>` appended when it names a file under
    /// `/assets/`. Anything else — an external URL, a file that doesn't
    /// exist — comes back unchanged.
    pub fn url(&self, path: &str) -> String {
        match self.version(path) {
            Some(version) => format!("{path}?v={version}"),
            None => path.to_string(),
        }
    }

    fn version(&self, path: &str) -> Option<String> {
        let relative = path.strip_prefix(URL_PREFIX)?;
        match &self.hashes {
            Some(hashes) => hashes.get(relative).cloned(),
            None => {
                if relative.split('/').any(|part| part == "..") {
                    return None;
                }
                hash_file(&self.root.join(relative))
            }
        }
    }
}

fn collect(root: &Path, dir: &Path, hashes: &mut HashMap<String, String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        warn!(dir = %dir.display(), "could not read static asset directory");
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, hashes);
        } else if let (Ok(relative), Some(hash)) = (path.strip_prefix(root), hash_file(&path)) {
            let key = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            hashes.insert(key, hash);
        }
    }
}

fn hash_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        .chars()
        .take(HASH_LEN)
        .collect()
}

/// Middleware for the `/assets` router. Debug builds never cache, so edits
/// show up on refresh.
pub async fn cache_control(
    State(state): State<SharedState>,
    request: Request,
    next: Next,
) -> Response {
    let header = if cfg!(debug_assertions) {
        "no-cache"
    } else {
        cache_policy(&state.assets, request.uri().path(), request.uri().query())
    };
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(header));
    response
}

fn cache_policy(assets: &AssetManifest, path: &str, query: Option<&str>) -> &'static str {
    let requested = query
        .unwrap_or_default()
        .split('&')
        .find_map(|pair| pair.strip_prefix("v="));
    match (requested, assets.version(path)) {
        (Some(requested), Some(current)) if requested == current => IMMUTABLE,
        _ => SHORT_LIVED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(files: &[(&str, &str)]) -> AssetManifest {
        AssetManifest {
            root: PathBuf::from("public"),
            hashes: Some(Arc::new(
                files
                    .iter()
                    .map(|(path, hash)| (path.to_string(), hash.to_string()))
                    .collect(),
            )),
        }
    }

    #[test]
    fn versions_known_assets_and_leaves_everything_else_alone() {
        let assets = manifest(&[("styles.css", "abc123")]);
        assert_eq!(
            assets.url("/assets/styles.css"),
            "/assets/styles.css?v=abc123"
        );
        assert_eq!(assets.url("/assets/missing.js"), "/assets/missing.js");
        assert_eq!(
            assets.url("https://example.com/a.png"),
            "https://example.com/a.png"
        );
    }

    #[test]
    fn only_the_current_version_is_cached_forever() {
        let assets = manifest(&[("styles.css", "abc123")]);
        assert_eq!(
            cache_policy(&assets, "/assets/styles.css", Some("v=abc123")),
            IMMUTABLE
        );
        assert_eq!(
            cache_policy(&assets, "/assets/styles.css", Some("v=stale0")),
            SHORT_LIVED
        );
        assert_eq!(
            cache_policy(&assets, "/assets/styles.css", None),
            SHORT_LIVED
        );
        assert_eq!(
            cache_policy(&assets, "/assets/images/flags/us.svg", None),
            SHORT_LIVED
        );
    }

    #[test]
    fn hashes_change_with_content() {
        assert_eq!(hash_bytes(b"a").len(), HASH_LEN);
        assert_eq!(hash_bytes(b"a"), hash_bytes(b"a"));
        assert_ne!(hash_bytes(b"a"), hash_bytes(b"b"));
    }

    #[test]
    fn hashes_every_file_under_public() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("public");
        let mut hashes = HashMap::new();
        collect(&root, &root, &mut hashes);
        assert!(hashes.contains_key("scripts/toast.js"));
        assert!(hashes.contains_key("images/app-icon.svg"));
    }

    #[test]
    fn debug_lookups_refuse_to_leave_public() {
        let assets = AssetManifest {
            root: Path::new(env!("CARGO_MANIFEST_DIR")).join("public"),
            hashes: None,
        };
        assert!(assets.version("/assets/scripts/toast.js").is_some());
        assert!(assets.version("/assets/../Cargo.toml").is_none());
    }

    /// Scripts and stylesheets linked straight from a template would skip
    /// cache busting, so every one has to go through `shared.asset(...)`.
    #[test]
    fn templates_link_scripts_and_styles_through_the_asset_helper() {
        let templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
        let mut offenders = Vec::new();
        visit_templates(&templates, &mut offenders);
        assert!(
            offenders.is_empty(),
            "use {{{{ shared.asset(\"/assets/…\") }}}} for: {offenders:#?}"
        );
    }

    fn visit_templates(dir: &Path, offenders: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit_templates(&path, offenders);
                continue;
            }
            let contents = std::fs::read_to_string(&path).unwrap();
            for (n, line) in contents.lines().enumerate() {
                for attr in ["src=\"/assets/", "href=\"/assets/"] {
                    let Some(start) = line.find(attr) else {
                        continue;
                    };
                    let url = line[start + attr.len()..].split('"').next().unwrap_or("");
                    if url.ends_with(".js") || url.ends_with(".css") {
                        offenders.push(format!("{}:{}", path.display(), n + 1));
                    }
                }
            }
        }
    }
}
