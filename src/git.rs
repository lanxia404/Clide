use tokio::process::Command;

#[derive(Clone)]
pub struct GitState {
    pub current_branch: String,
    pub status: Vec<String>,
}

impl GitState {
    pub fn new() -> Self {
        Self {
            current_branch: "unknown".to_string(),
            status: vec!["Fetching status...".to_string()],
        }
    }

    pub async fn update(&mut self) {
        let branch_output = Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .output()
            .await;

        if let Ok(output) = branch_output {
            if output.status.success() {
                self.current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            } else {
                self.current_branch = "not a git repo".to_string();
                self.status = vec!["Not inside a Git repository.".to_string()];
                return;
            }
        }

        let status_output = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .output()
            .await;
        
        if let Ok(output) = status_output {
            if output.status.success() {
                let status_text = String::from_utf8_lossy(&output.stdout);
                if status_text.trim().is_empty() {
                    self.status = vec!["No changes.".to_string()];
                } else {
                    self.status = status_text.trim().lines().map(String::from).collect();
                }
            }
        }
    }
}