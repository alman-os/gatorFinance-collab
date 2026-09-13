use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{AppResult, message};
use crate::models::{
    ARTIFACT_SCHEMA_VERSION, ArtifactActionResult, CategoryTotal, ENGINE_VERSION, GatorArtifact,
    HealthResult, LibraryItem, LibraryScan, ReportPayload, ReportPrivacy, SharePayload,
};

#[derive(Clone)]
pub struct OutputLibrary {
    root: PathBuf,
}

impl OutputLibrary {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure(&self) -> AppResult<()> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    pub fn scan(&self) -> AppResult<LibraryScan> {
        self.ensure()?;
        let mut items = Vec::new();
        let mut skipped_invalid = 0usize;
        let mut duplicate_payloads = 0usize;
        let mut hashes = HashSet::new();
        let mut paths = fs::read_dir(&self.root)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.ends_with(".gatorfinance.json"))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        paths.sort();

        for path in paths {
            match self.read_artifact(&path) {
                Ok(artifact) => {
                    let hash = payload_hash(&artifact)?;
                    if !hashes.insert(hash.clone()) {
                        duplicate_payloads += 1;
                    }
                    items.push(item_from_artifact(&artifact, &path, hash));
                }
                Err(_) => skipped_invalid += 1,
            }
        }
        items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(LibraryScan {
            items,
            skipped_invalid,
            duplicate_payloads,
        })
    }

    pub fn save_report(
        &self,
        title: &str,
        currency: &str,
        health: HealthResult,
        categories: Vec<CategoryTotal>,
        transaction_count: usize,
    ) -> AppResult<ArtifactActionResult> {
        self.ensure()?;
        let period = period_label(&health);
        let artifact = GatorArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            artifact_id: Uuid::new_v4().to_string(),
            source_app: "gatorFinance".to_string(),
            engine_version: ENGINE_VERSION.to_string(),
            title: title.trim().to_string(),
            created_at: Utc::now().to_rfc3339(),
            payload_type: "financial_health_report".to_string(),
            currency: currency.to_string(),
            period,
            privacy: ReportPrivacy {
                contains_account_identifiers: false,
                contains_transaction_details: false,
                safe_for_ai_share: true,
            },
            payload: ReportPayload {
                health,
                categories,
                transaction_count,
            },
        };
        validate(&artifact)?;
        let hash = payload_hash(&artifact)?;
        if self
            .scan()?
            .items
            .iter()
            .any(|item| item.payload_hash == hash)
        {
            return Err(message("An identical report already exists in the library"));
        }
        let path = unique_path(&self.root, &safe_filename(&artifact.title));
        write_atomic(&path, &serde_json::to_vec_pretty(&artifact)?)?;
        let item = item_from_artifact(&artifact, &path, hash);
        Ok(ArtifactActionResult {
            message: "[SAVED TO LIBRARY]".to_string(),
            path: Some(path.to_string_lossy().to_string()),
            item: Some(item),
        })
    }

    pub fn import_artifact(&self, source: &Path) -> AppResult<ArtifactActionResult> {
        self.ensure()?;
        let valid_suffix = source
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".gatorfinance.json"));
        if !valid_suffix {
            return Err(message("Reports must use the .gatorfinance.json suffix"));
        }
        let artifact = self.read_artifact(source)?;
        let hash = payload_hash(&artifact)?;
        if self
            .scan()?
            .items
            .iter()
            .any(|item| item.payload_hash == hash)
        {
            return Err(message("That report is already in the library"));
        }
        let path = unique_path(&self.root, &safe_filename(&artifact.title));
        write_atomic(&path, &serde_json::to_vec_pretty(&artifact)?)?;
        Ok(ArtifactActionResult {
            message: "[IMPORTED]".to_string(),
            path: Some(path.to_string_lossy().to_string()),
            item: Some(item_from_artifact(&artifact, &path, hash)),
        })
    }

    pub fn trash_artifact(&self, path: &Path) -> AppResult<ArtifactActionResult> {
        let canonical_root = self.root.canonicalize()?;
        let canonical_path = path.canonicalize()?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(message(
                "Only gatorFinance library files can be moved to Trash",
            ));
        }
        self.read_artifact(&canonical_path)?;
        trash::delete(&canonical_path)
            .map_err(|error| message(format!("Could not move report to Trash: {error}")))?;
        Ok(ArtifactActionResult {
            message: "[MOVED TO TRASH]".to_string(),
            path: None,
            item: None,
        })
    }

    pub fn serialized(&self, path: &Path) -> AppResult<String> {
        let artifact = self.read_artifact(path)?;
        Ok(serde_json::to_string_pretty(&artifact)?)
    }

    pub fn share_payload(&self, path: &Path, provider: &str) -> AppResult<SharePayload> {
        let artifact = self.read_artifact(path)?;
        if !artifact.privacy.safe_for_ai_share {
            return Err(message("This report is not marked safe for AI sharing"));
        }
        let serialized = serde_json::to_string_pretty(&artifact)?;
        let text = format!(
            "Context:\nThis payload was generated by gatorFinance. Help interpret the financial health report while preserving the artifact schema.\n\nTask:\nReview the report, identify the strongest and weakest signals, and suggest practical questions for the user to investigate.\n\nArtifact:\n<here>\n{serialized}\n</here>\n\nOutput format:\nReturn concise observations and an updated artifact only if requested. Preserve the same schema. Do not add Markdown headings that start with the pound/hash symbol."
        );
        let url = match provider {
            "chatgpt" => "https://chatgpt.com/",
            "claude" => "https://claude.ai/new",
            _ => return Err(message("Unknown AI provider")),
        };
        Ok(SharePayload {
            url: url.to_string(),
            text,
        })
    }

    fn read_artifact(&self, path: &Path) -> AppResult<GatorArtifact> {
        let artifact: GatorArtifact = serde_json::from_str(&fs::read_to_string(path)?)?;
        validate(&artifact)?;
        Ok(artifact)
    }
}

fn validate(artifact: &GatorArtifact) -> AppResult<()> {
    if artifact.schema_version != ARTIFACT_SCHEMA_VERSION {
        return Err(message(format!(
            "Unsupported schema version {}",
            artifact.schema_version
        )));
    }
    if artifact.source_app != "gatorFinance" || artifact.payload_type != "financial_health_report" {
        return Err(message("File is not a gatorFinance health report"));
    }
    if artifact.title.trim().is_empty() || artifact.artifact_id.trim().is_empty() {
        return Err(message("Artifact title and ID are required"));
    }
    Ok(())
}

fn payload_hash(artifact: &GatorArtifact) -> AppResult<String> {
    let bytes = serde_json::to_vec(&artifact.payload)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn item_from_artifact(artifact: &GatorArtifact, path: &Path, payload_hash: String) -> LibraryItem {
    LibraryItem {
        artifact_id: artifact.artifact_id.clone(),
        title: artifact.title.clone(),
        created_at: artifact.created_at.clone(),
        period_label: artifact.period.clone(),
        period_start: artifact.payload.health.period_start,
        period_end: artifact.payload.health.period_end,
        score: artifact.payload.health.score,
        grade: artifact.payload.health.grade.clone(),
        path: path.to_string_lossy().to_string(),
        payload_hash,
    }
}

fn period_label(health: &HealthResult) -> String {
    match (health.period_start, health.period_end) {
        (Some(start), Some(end)) => format!("{} – {}", start.format("%b %Y"), end.format("%b %Y")),
        _ => "No period".to_string(),
    }
}

fn safe_filename(title: &str) -> String {
    let sanitized = title
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '\0'..='\u{1f}' => '-',
            _ => character,
        })
        .collect::<String>();
    let compact = sanitized.split_whitespace().collect::<Vec<_>>().join("-");
    let bounded = compact.chars().take(80).collect::<String>();
    if bounded.is_empty() {
        "gator-health-report".to_string()
    } else {
        bounded
    }
}

fn unique_path(root: &Path, stem: &str) -> PathBuf {
    let first = root.join(format!("{stem}.gatorfinance.json"));
    if !first.exists() {
        return first;
    }
    for index in 2..10_000 {
        let candidate = root.join(format!("{stem}-{index}.gatorfinance.json"));
        if !candidate.exists() {
            return candidate;
        }
    }
    root.join(format!("{stem}-{}.gatorfinance.json", Uuid::new_v4()))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::calculate_health;

    #[test]
    fn share_payload_opens_provider_and_has_no_hash_headings() {
        let root =
            std::env::temp_dir().join(format!("gatorfinance-library-test-{}", Uuid::new_v4()));
        let library = OutputLibrary::new(root.clone());
        let saved = library
            .save_report("Test Report", "USD", calculate_health(&[]), Vec::new(), 0)
            .unwrap();
        let path = PathBuf::from(saved.path.unwrap());
        let share = library.share_payload(&path, "chatgpt").unwrap();
        assert_eq!(share.url, "https://chatgpt.com/");
        assert!(!share.text.lines().any(|line| line.starts_with('#')));
        assert_eq!(library.scan().unwrap().items.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_files_are_visible_in_scan_count() {
        let root =
            std::env::temp_dir().join(format!("gatorfinance-library-invalid-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("bad.gatorfinance.json"), "{}").unwrap();
        let scan = OutputLibrary::new(root.clone()).scan().unwrap();
        assert_eq!(scan.skipped_invalid, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_requires_the_artifact_suffix() {
        let root =
            std::env::temp_dir().join(format!("gatorfinance-library-suffix-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("report.json");
        fs::write(&source, "{}").unwrap();
        let error = OutputLibrary::new(root.clone())
            .import_artifact(&source)
            .unwrap_err();
        assert!(error.to_string().contains(".gatorfinance.json"));
        let _ = fs::remove_dir_all(root);
    }
}
