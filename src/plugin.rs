use crate::app::App;
use anyhow::Result;

/// The core trait for any Clide plugin.
/// Defines the callbacks that the plugin system will invoke.
pub trait Plugin {
    /// Returns the name of the plugin.
    fn name(&self) -> &'static str;

    /// Called once when the plugin is loaded.
    /// Can be used for initialization.
    fn on_load(&mut self, _app: &mut App) -> Result<()> {
        Ok(())
    }

    /// Called on every application tick.
    fn on_tick(&self) -> Result<()> {
        Ok(())
    }

    // Add other hooks as needed, for example:
    // fn on_key_event(&mut self, app: &mut App, key_event: &KeyEvent) -> Result<()>;
    // fn on_file_open(&mut self, app: &mut App, file_path: &Path) -> Result<()>;
}

/// Manages the lifecycle of all loaded plugins.
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// A placeholder for the plugin loading mechanism.
    pub fn load_plugins(&mut self) {
        // In the future, this would dynamically load plugins from a directory,
        // for example, from .so or .dll files.
        // For now, we can't add any plugins as we don't have a concrete implementation.
    }

    // Example of how you would call a hook on all plugins
    pub fn tick_plugins(&self) {
        for plugin in &self.plugins {
            let _ = plugin.on_tick();
        }
    }
}
