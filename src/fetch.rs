use crate::config::{
    DOWNLOADS, LIBCHEWING_SOURCE_ID, MOZC_EMOTICON_SOURCE_ID, RIME_ESSAY_SOURCE_ID,
};
use crate::files::{sha256_bytes, write_tree_inventory};
use anyhow::{bail, Context, Result};
use std::fs;
use std::process::Command;

pub fn run() -> Result<()> {
    let root = std::env::current_dir().context("read current directory")?;
    for source in DOWNLOADS {
        let bytes = curl(source.url).with_context(|| format!("fetch {}", source.url))?;
        let actual = sha256_bytes(&bytes);
        if actual != source.sha256 {
            bail!(
                "checksum mismatch for {}\n  expected: {}\n  actual:   {}",
                source.url,
                source.sha256,
                actual
            );
        }

        let target = root.join(source.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(&target, bytes).with_context(|| format!("write {}", target.display()))?;
        println!("fetched {}", source.path);
    }

    write_tree_inventory(&root, LIBCHEWING_SOURCE_ID)?;
    write_tree_inventory(&root, RIME_ESSAY_SOURCE_ID)?;
    write_tree_inventory(&root, MOZC_EMOTICON_SOURCE_ID)?;
    println!("modern source fetch complete");
    Ok(())
}

fn curl(url: &str) -> Result<Vec<u8>> {
    let output = Command::new("curl")
        .args(["-fsSL", "-A", "chiakey-lexicon-fetcher", url])
        .output()
        .context("run curl")?;
    if !output.status.success() {
        bail!(
            "curl failed for {}: {}",
            url,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}
