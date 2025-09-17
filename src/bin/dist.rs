use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

struct BuildPlan {
    triple: Option<&'static str>,
    path_components: Vec<&'static str>,
    dest_name: &'static str,
    mark_executable: bool,
    label: &'static str,
}

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dist_dir = manifest_dir.join("dist");

    if dist_dir.exists() {
        fs::remove_dir_all(&dist_dir)
            .with_context(|| format!("無法清空輸出目錄：{}", dist_dir.display()))?;
    }
    fs::create_dir_all(&dist_dir)
        .with_context(|| format!("無法建立輸出目錄：{}", dist_dir.display()))?;

    let host_is_windows = cfg!(target_os = "windows");
    let host_is_linux = cfg!(target_os = "linux");

    let mut plans = Vec::new();
    let host_bin = if host_is_windows {
        "clide.exe"
    } else {
        "clide"
    };
    plans.push(BuildPlan {
        triple: None,
        path_components: vec!["target", "release", host_bin],
        dest_name: host_bin,
        mark_executable: !host_is_windows,
        label: if host_is_windows {
            "建置 Windows 原生版本"
        } else {
            "建置 Linux 原生版本"
        },
    });

    if host_is_linux {
        plans.push(BuildPlan {
            triple: Some("x86_64-pc-windows-gnu"),
            path_components: vec!["target", "x86_64-pc-windows-gnu", "release", "clide.exe"],
            dest_name: "clide.exe",
            mark_executable: false,
            label: "交叉建置 Windows 版本",
        });
    }

    for plan in plans {
        build_target(&manifest_dir, &dist_dir, plan)?;
    }

    println!(">> 已輸出執行檔至 {}", dist_dir.display());
    Ok(())
}

fn build_target(manifest_dir: &Path, dist_dir: &Path, plan: BuildPlan) -> Result<()> {
    println!(">> {}", plan.label);
    let mut command = Command::new("cargo");
    command.arg("build").arg("--release");
    if let Some(triple) = plan.triple {
        command.arg("--target").arg(triple);
    }
    let status = command
        .current_dir(manifest_dir)
        .status()
        .with_context(|| "無法執行 cargo build")?;
    if !status.success() {
        if let Some(triple) = plan.triple {
            bail!(
                "cargo build 失敗，請先執行 `rustup target add {}` 並確認交叉工具鏈可用",
                triple
            );
        }
        bail!("cargo build 失敗");
    }

    let mut src_path = PathBuf::new();
    for component in plan.path_components {
        src_path.push(component);
    }
    let src_path = manifest_dir.join(src_path);
    if !src_path.exists() {
        bail!(
            "未找到建置結果：{}，請確認對應 target 已安裝",
            src_path.display()
        );
    }

    let dest_path = dist_dir.join(plan.dest_name);
    fs::copy(&src_path, &dest_path).with_context(|| {
        format!(
            "無法複製檔案至 {} <- {}",
            dest_path.display(),
            src_path.display()
        )
    })?;

    if plan.mark_executable {
        make_executable(&dest_path)?;
    }

    println!(">> 完成：{} -> {}", src_path.display(), dest_path.display());
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("無法讀取權限：{}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("無法設定權限：{}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<()> {
    Ok(())
}
