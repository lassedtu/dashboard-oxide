//! Extensions are bundles of actions that can be switched on and off from
//! Config → Extensions (stored as `[extensions] disabled` in config.toml).
//!
//! Everything optional in dbx is an extension, including the built-in ones
//! below. Config and Quit are core actions and always present.

mod new_project;
mod recent_projects;

use crate::actions::Action;
use crate::config::Config;

pub struct Extension {
    /// Stable id, used in `[extensions] disabled`. Never change it once released.
    pub id: String,
    pub name: String,
    /// One short line, shown in the Extensions list.
    pub description: String,
    /// Added to the main menu, in this order, when the extension is enabled.
    pub actions: Vec<Box<dyn Action>>,
}

impl Extension {
    pub fn is_enabled(&self, config: &Config) -> bool {
        !config.extensions.disabled.contains(&self.id)
    }
}

/// Every known extension, enabled or not, in menu order.
pub fn all() -> Vec<Extension> {
    builtin()
}

fn builtin() -> Vec<Extension> {
    vec![
        Extension {
            id: "new-project".into(),
            name: "New project".into(),
            description: "Create a project folder".into(),
            actions: vec![Box::new(new_project::NewProject)],
        },
        Extension {
            id: "recent-projects".into(),
            name: "Recent projects".into(),
            description: "Open a recent project".into(),
            actions: vec![Box::new(recent_projects::RecentProjects)],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let all = all();
        let ids: HashSet<_> = all.iter().map(|e| &e.id).collect();
        assert_eq!(ids.len(), all.len());
    }
}
