use crate::error::{OrqError, Result};
use crate::store::Store;
use crate::types::*;
use std::collections::HashMap;

pub struct ModelRegistry<'a> {
    store: &'a Store,
}

impl<'a> ModelRegistry<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn add(
        &self,
        workspace: &str,
        id: &str,
        display_name: Option<&str>,
        capabilities: Vec<String>,
        cost_weight: f64,
        latency_weight: f64,
        recipe: LaunchRecipe,
    ) -> Result<Model> {
        self.store.ensure_workspace(workspace, None)?;
        let model = Model {
            workspace: workspace.into(),
            id: id.into(),
            display_name: display_name.unwrap_or(id).into(),
            capabilities,
            cost_weight,
            latency_weight,
            recipe,
            created_at: now(),
        };
        self.store.upsert_model(&model)?;
        Ok(model)
    }

    pub fn add_cli(
        &self,
        workspace: &str,
        id: &str,
        template: &str,
        capabilities: Vec<String>,
    ) -> Result<Model> {
        self.add(
            workspace,
            id,
            None,
            capabilities,
            1.0,
            1.0,
            LaunchRecipe {
                kind: LaunchKind::Cli,
                template: template.into(),
                model_arg: Some(id.into()),
                env: HashMap::new(),
            },
        )
    }

    pub fn get(&self, workspace: &str, id: &str) -> Result<Model> {
        self.store
            .get_model(workspace, id)?
            .ok_or_else(|| OrqError::Other(format!("model not found: {id}")))
    }

    pub fn list(&self, workspace: &str) -> Result<Vec<Model>> {
        self.store.list_models(workspace)
    }

    /// Expand launch recipe into a shell command line.
    pub fn expand_command(model: &Model, user_cmd: &str, prompt_file: Option<&str>) -> String {
        let model_arg = model
            .recipe
            .model_arg
            .as_deref()
            .unwrap_or(model.id.as_str());
        let pf = prompt_file.unwrap_or("");
        match model.recipe.kind {
            LaunchKind::Cli => model
                .recipe
                .template
                .replace("{cmd}", user_cmd)
                .replace("{model}", model_arg)
                .replace("{prompt_file}", pf),
            LaunchKind::HttpStub => {
                // Runner receives request file path via {prompt_file}; body written by job layer
                model
                    .recipe
                    .template
                    .replace("{cmd}", user_cmd)
                    .replace("{model}", model_arg)
                    .replace("{prompt_file}", pf)
            }
        }
    }
}
