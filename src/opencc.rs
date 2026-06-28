use anyhow::{bail, Context, Result};
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

pub fn convert_lines(binary: &Path, config: &Path, input: &[String]) -> Result<Vec<String>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    ensure_opencc_cli(binary)?;

    let mut child = Command::new(binary)
        .arg("-c")
        .arg(config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn OpenCC CLI {}", binary.display()))?;

    let expected_len = input.len();
    let mut stdin = child.stdin.take().context("open OpenCC stdin")?;
    let input = input.to_owned();
    let writer = thread::spawn(move || -> Result<()> {
        for line in input {
            stdin
                .write_all(line.as_bytes())
                .context("write phrase to OpenCC stdin")?;
            stdin.write_all(b"\n").context("write OpenCC newline")?;
        }
        Ok(())
    });

    let output = child.wait_with_output().context("wait for OpenCC CLI")?;
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("OpenCC stdin writer thread panicked"))??;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "OpenCC CLI failed with {}: {}",
            output.status,
            stderr.trim()
        );
    }

    let text = String::from_utf8(output.stdout).context("decode OpenCC stdout")?;
    let converted = text.lines().map(str::to_string).collect::<Vec<_>>();
    if converted.len() != expected_len {
        bail!(
            "OpenCC returned {} lines for {} input lines",
            converted.len(),
            expected_len
        );
    }
    Ok(converted)
}

fn ensure_opencc_cli(binary: &Path) -> Result<()> {
    if command_exists(binary) {
        return Ok(());
    }

    let binary_name = binary.file_name().and_then(|name| name.to_str());
    if binary_name != Some("opencc") {
        bail!(
            "OpenCC CLI {} was not found. Install OpenCC or set OPENCC_BINARY to a valid executable.",
            binary.display()
        );
    }

    if !io::stdin().is_terminal() {
        bail!(
            "OpenCC CLI was not found. Install it with `brew install opencc`, or set OPENCC_BINARY to a valid executable."
        );
    }

    eprint!("OpenCC CLI was not found. Install it with Homebrew now? [y/N] ");
    io::stderr()
        .flush()
        .context("flush OpenCC install prompt")?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("read OpenCC install prompt answer")?;
    if !is_yes(&answer) {
        bail!(
            "OpenCC CLI is required. Install it with `brew install opencc`, or set OPENCC_BINARY to a valid executable."
        );
    }

    if !command_exists(Path::new("brew")) {
        bail!("Homebrew was not found. Install OpenCC manually, or set OPENCC_BINARY to a valid executable.");
    }

    let status = Command::new("brew")
        .arg("install")
        .arg("opencc")
        .status()
        .context("run `brew install opencc`")?;
    if !status.success() {
        bail!("`brew install opencc` failed with {status}");
    }

    if !command_exists(binary) {
        bail!(
            "OpenCC CLI still was not found after installation. Set OPENCC_BINARY to the installed executable path."
        );
    }

    Ok(())
}

fn command_exists(binary: &Path) -> bool {
    match Command::new(binary)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

fn is_yes(answer: &str) -> bool {
    matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "是" | "對"
    )
}

#[cfg(test)]
mod tests {
    use super::is_yes;

    #[test]
    fn parses_install_confirmation() {
        assert!(is_yes("y"));
        assert!(is_yes("YES\n"));
        assert!(is_yes("是"));
        assert!(is_yes("對"));
        assert!(!is_yes(""));
        assert!(!is_yes("n"));
        assert!(!is_yes("no"));
    }
}
