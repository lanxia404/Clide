use tokio::process::Command;

#[derive(Clone)]
pub struct GitState {
    pub current_branch: String,
    pub status: Vec<String>,
}

impl Default for GitState {
    fn default() -> Self {
        Self::new()
    }
}

impl GitState {
    pub fn new() -> Self {
        Self {
            current_branch: "unknown".to_string(),
            status: vec!["Fetching status...".to_string()],
        }
    }

    pub async fn update(&mut self) {
        // Reset state before fetching
        self.current_branch = "unknown".to_string();
        self.status = vec![];

        let branch_output = Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .output()
            .await;

        match branch_output {
            Ok(output) if output.status.success() => {
                self.current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
            _ => {
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
        
        match status_output {
            Ok(output) if output.status.success() => {
                let status_text = String::from_utf8_lossy(&output.stdout);
                if status_text.trim().is_empty() {
                    self.status = vec!["No changes.".to_string()];
                } else {
                    self.status = status_text.trim().lines().map(String::from).collect();
                }
            }
            _ => {
                self.status = vec!["Failed to get git status.".to_string()];
            }
        }
    }
}