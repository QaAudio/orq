use crate::affinity::AffinityEngine;
use crate::error::{OrqError, Result};
use crate::eval::Evaluator;
use crate::model::ModelRegistry;
use crate::router::Router;
use crate::store::Store;
use crate::task::TaskEngine;
use crate::types::*;
use serde_json::json;
use std::collections::HashMap;

pub struct JobEngine<'a> {
    store: &'a Store,
}

pub struct JobSubmitOpts<'a> {
    pub workspace: &'a str,
    pub session: Option<&'a str>,
    pub name: &'a str,
    pub command: &'a str,
    pub claims: Vec<String>,
    pub class_override: Option<&'a str>,
    pub strategy: ScheduleStrategy,
    pub policy: RoutePolicy,
    pub k: u32,
    pub moa_layers: u32,
    pub aggregator: Option<&'a str>,
    pub seed: Option<u64>,
    pub capability: Option<&'a str>,
    pub force_model: Option<&'a str>,
    pub cwd: Option<&'a str>,
}

impl<'a> JobEngine<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn submit(&self, opts: JobSubmitOpts<'_>) -> Result<Job> {
        self.store.ensure_workspace(opts.workspace, None)?;
        let eval = Evaluator::new(self.store).evaluate(
            opts.workspace,
            opts.name,
            opts.command,
            &opts.claims,
            &[],
            opts.class_override,
        )?;

        let k = match opts.strategy {
            ScheduleStrategy::Single => 1,
            ScheduleStrategy::Race | ScheduleStrategy::Moa => opts.k.max(1),
        };

        let decision = Router::new(self.store).decide(
            opts.workspace,
            &eval.class,
            opts.policy,
            k,
            opts.seed,
            opts.capability,
            opts.aggregator,
            opts.force_model,
        )?;

        let job = Job {
            id: new_id(),
            workspace: opts.workspace.into(),
            session: opts.session.map(|s| s.to_string()),
            name: opts.name.into(),
            command: opts.command.into(),
            class: eval.class.clone(),
            strategy: opts.strategy,
            policy: opts.policy,
            status: JobStatus::Planned,
            seed: decision.seed,
            epoch: decision.epoch,
            k,
            moa_layers: opts.moa_layers.max(1),
            aggregator_model: decision.aggregator.clone(),
            claims: opts.claims.clone(),
            features: eval.features,
            route_reason: decision.reason.clone(),
            winner_task_id: None,
            current_layer: 0,
            created_at: now(),
            updated_at: now(),
        };
        self.store.insert_job(&job)?;

        // Ensure moa POI table
        let _ = self.store.create_poi_table(
            opts.workspace,
            "moa",
            "generic",
            vec![
                ColumnDef {
                    name: "body".into(),
                    col_type: "string".into(),
                    poi: true,
                },
                ColumnDef {
                    name: "model".into(),
                    col_type: "string".into(),
                    poi: false,
                },
            ],
        );

        match opts.strategy {
            ScheduleStrategy::Single => {
                self.spawn_model_task(&job, &decision.models[0], "worker", 0, opts.cwd, None)?;
            }
            ScheduleStrategy::Race => {
                for m in &decision.models {
                    self.spawn_model_task(&job, m, "racer", 0, opts.cwd, None)?;
                }
            }
            ScheduleStrategy::Moa => {
                self.spawn_moa_layer(&job, &decision.models, 0, opts.cwd)?;
            }
        }

        let mut job = self
            .store
            .get_job(&job.id)?
            .ok_or_else(|| OrqError::Other("job missing".into()))?;
        job.status = JobStatus::Running;
        job.updated_at = now();
        self.store.update_job(&job)?;
        Ok(job)
    }

    fn spawn_model_task(
        &self,
        job: &Job,
        model_id: &str,
        role: &str,
        layer: u32,
        cwd: Option<&str>,
        prompt_file: Option<&str>,
    ) -> Result<Task> {
        let model = ModelRegistry::new(self.store).get(&job.workspace, model_id)?;
        let cmd = ModelRegistry::expand_command(&model, &job.command, prompt_file);
        let engine = TaskEngine::new(self.store);
        let mut task = engine.submit(
            &job.workspace,
            job.session.as_deref(),
            &format!("{}-{}-L{layer}", job.name, model_id),
            &cmd,
            TaskKind::Oneshot,
            "shell",
            job.claims.clone(),
            vec![],
            vec![],
            RestartPolicy::Never,
            1,
            cwd,
        )?;
        task.job_id = Some(job.id.clone());
        task.model_id = Some(model_id.into());
        task.class = Some(job.class.clone());
        task.role = Some(role.into());
        self.store.update_task(&task)?;
        self.store
            .add_job_child(&job.id, &task.id, role, layer)?;
        Ok(task)
    }

    fn spawn_moa_layer(
        &self,
        job: &Job,
        models: &[String],
        layer: u32,
        cwd: Option<&str>,
    ) -> Result<()> {
        // Optional: write previous layer summary into a prompt file for http_stub / cli
        let prompt_file = self.write_layer_prompt(job, layer)?;
        for m in models {
            self.spawn_model_task(
                job,
                m,
                "proposer",
                layer,
                cwd,
                prompt_file.as_deref(),
            )?;
        }
        Ok(())
    }

    fn write_layer_prompt(&self, job: &Job, layer: u32) -> Result<Option<String>> {
        if layer == 0 {
            return Ok(None);
        }
        let children = self.store.job_children(&job.id)?;
        let mut parts = Vec::new();
        for (tid, role, lay) in children {
            if lay + 1 != layer || role != "proposer" && role != "aggregator" {
                // collect previous layer proposers (layer-1)
                if lay != layer - 1 {
                    continue;
                }
            }
            if lay != layer.saturating_sub(1) {
                continue;
            }
            if let Ok(Some(t)) = self.store.get_task(&tid) {
                let log = t
                    .log_path
                    .as_ref()
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .unwrap_or_default();
                parts.push(format!(
                    "### model={} task={}\n{log}\n",
                    t.model_id.unwrap_or_default(),
                    tid
                ));
            }
        }
        if parts.is_empty() {
            return Ok(None);
        }
        let path = self
            .store
            .data_dir
            .join("tmp")
            .join(format!("moa-{}-L{layer}.txt", &job.id[..8.min(job.id.len())]));
        std::fs::write(&path, parts.join("\n---\n"))?;
        Ok(Some(path.display().to_string()))
    }

    /// Drive job to completion synchronously (runs child tasks, race cancel, moa aggregate).
    pub fn run_sync(&self, job_id: &str) -> Result<Job> {
        let engine = TaskEngine::new(self.store);
        loop {
            let job = self
                .store
                .get_job(job_id)?
                .ok_or_else(|| OrqError::Other("job not found".into()))?;
            if job.status.is_terminal() {
                return Ok(job);
            }

            // Start/run any queued children for this job
            let tasks = self.store.list_tasks_for_job(job_id)?;
            for t in &tasks {
                if t.status == TaskStatus::Queued || t.status == TaskStatus::Blocked {
                    let _ = engine.run_to_completion(&t.id);
                } else if t.status == TaskStatus::Starting || t.status == TaskStatus::Running {
                    // shouldn't happen in sync path
                    let _ = engine.run_to_completion(&t.id);
                }
            }

            self.tick_job(job_id)?;
        }
    }

    pub fn tick_job(&self, job_id: &str) -> Result<Job> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| OrqError::Other("job not found".into()))?;
        if job.status.is_terminal() {
            return Ok(job);
        }

        let children = self.store.job_children(job_id)?;
        let tasks: HashMap<String, Task> = self
            .store
            .list_tasks_for_job(job_id)?
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();

        match job.strategy {
            ScheduleStrategy::Single => {
                if let Some((tid, _, _)) = children.first() {
                    if let Some(t) = tasks.get(tid) {
                        if t.status.is_terminal() {
                            self.finalize_job(&mut job, t)?;
                        }
                    }
                }
            }
            ScheduleStrategy::Race => {
                let mut winner: Option<&Task> = None;
                for (tid, role, _) in &children {
                    if role != "racer" {
                        continue;
                    }
                    if let Some(t) = tasks.get(tid) {
                        if t.status == TaskStatus::Done {
                            winner = Some(t);
                            break;
                        }
                    }
                }
                if let Some(w) = winner {
                    // cancel others
                    let engine = TaskEngine::new(self.store);
                    for (tid, role, _) in &children {
                        if role == "racer" && tid != &w.id {
                            if let Some(t) = tasks.get(tid) {
                                if !t.status.is_terminal() {
                                    let _ = engine.cancel(tid, true);
                                }
                            }
                        }
                    }
                    self.finalize_job(&mut job, w)?;
                } else {
                    // all failed?
                    let racers: Vec<_> = children
                        .iter()
                        .filter(|(_, r, _)| r == "racer")
                        .filter_map(|(tid, _, _)| tasks.get(tid))
                        .collect();
                    if !racers.is_empty() && racers.iter().all(|t| t.status.is_terminal()) {
                        job.status = JobStatus::Failed;
                        job.updated_at = now();
                        self.store.update_job(&job)?;
                    }
                }
            }
            ScheduleStrategy::Moa => {
                self.tick_moa(&mut job, &children, &tasks)?;
            }
        }
        self.store
            .get_job(job_id)?
            .ok_or_else(|| OrqError::Other("job not found".into()))
    }

    fn tick_moa(
        &self,
        job: &mut Job,
        children: &[(String, String, u32)],
        tasks: &HashMap<String, Task>,
    ) -> Result<()> {
        let layer = job.current_layer;
        let proposers: Vec<_> = children
            .iter()
            .filter(|(_, role, lay)| role == "proposer" && *lay == layer)
            .collect();
        if proposers.is_empty() {
            return Ok(());
        }
        let all_done = proposers.iter().all(|(tid, _, _)| {
            tasks
                .get(tid.as_str())
                .map(|t| t.status.is_terminal())
                .unwrap_or(false)
        });
        if !all_done {
            return Ok(());
        }

        // Persist proposer outputs as POIs
        for (tid, _, _) in &proposers {
            if let Some(t) = tasks.get(tid.as_str()) {
                let body = t
                    .log_path
                    .as_ref()
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .unwrap_or_default();
                let key = format!("{}/layer{}/{}", job.id, layer, t.model_id.as_deref().unwrap_or("m"));
                let mut cols = HashMap::new();
                cols.insert(
                    "model".into(),
                    json!(t.model_id.clone().unwrap_or_default()),
                );
                let _ = self.store.set_poi(
                    &job.workspace,
                    "moa",
                    &key,
                    json!(body),
                    cols,
                    Some("proposed"),
                    StorageTier::Ephemeral,
                    job.session.as_deref(),
                    None,
                    None,
                );
            }
        }

        self.store.append_event(
            &job.workspace,
            job.session.as_deref(),
            "job.layer_done",
            json!({ "id": job.id, "layer": layer }),
        )?;

        // Aggregator for this layer
        let agg_model = job
            .aggregator_model
            .clone()
            .ok_or_else(|| OrqError::Other("moa missing aggregator".into()))?;

        let already_agg = children
            .iter()
            .any(|(_, role, lay)| role == "aggregator" && *lay == layer);
        if !already_agg {
            job.status = JobStatus::Reconciling;
            job.updated_at = now();
            self.store.update_job(job)?;

            let prompt = self.write_layer_prompt(job, layer + 1)?;
            // For aggregator, enrich command
            let mut job_for_agg = job.clone();
            job_for_agg.command = format!(
                "{} # aggregate layer {} proposals",
                job.command, layer
            );
            let task = self.spawn_model_task(
                &job_for_agg,
                &agg_model,
                "aggregator",
                layer,
                None,
                prompt.as_deref(),
            )?;
            // run immediately if sync caller will loop; here just queued
            let _ = task;
            return Ok(());
        }

        // Wait for aggregator
        let agg = children
            .iter()
            .find(|(_, role, lay)| role == "aggregator" && *lay == layer)
            .and_then(|(tid, _, _)| tasks.get(tid));
        let Some(agg) = agg else { return Ok(()) };
        if !agg.status.is_terminal() {
            return Ok(());
        }

        // Store aggregate POI
        let body = agg
            .log_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let key = format!("{}/layer{}/aggregate", job.id, layer);
        let _ = self.store.set_poi(
            &job.workspace,
            "moa",
            &key,
            json!(body),
            HashMap::new(),
            Some("aggregated"),
            StorageTier::Ephemeral,
            job.session.as_deref(),
            None,
            None,
        );

        if layer + 1 < job.moa_layers {
            // next propose layer using same models as initial route
            let decision_models: Vec<String> = children
                .iter()
                .filter(|(_, role, lay)| role == "proposer" && *lay == 0)
                .filter_map(|(tid, _, _)| {
                    tasks.get(tid).and_then(|t| t.model_id.clone())
                })
                .collect();
            job.current_layer = layer + 1;
            job.status = JobStatus::Running;
            job.updated_at = now();
            self.store.update_job(job)?;
            self.spawn_moa_layer(job, &decision_models, job.current_layer, None)?;
        } else {
            self.store.append_event(
                &job.workspace,
                job.session.as_deref(),
                "job.reconciled",
                json!({ "id": job.id, "winner": agg.id }),
            )?;
            self.finalize_job(job, agg)?;
        }
        Ok(())
    }

    fn finalize_job(&self, job: &mut Job, winner: &Task) -> Result<()> {
        job.winner_task_id = Some(winner.id.clone());
        job.status = if winner.status == TaskStatus::Done {
            JobStatus::Done
        } else {
            JobStatus::Failed
        };
        job.updated_at = now();
        self.store.update_job(job)?;

        // Affinity feedback for all children with model_id
        let aff = AffinityEngine::new(self.store);
        for t in self.store.list_tasks_for_job(&job.id)? {
            if let (Some(mid), Some(class)) = (t.model_id.as_deref(), t.class.as_deref()) {
                let success = t.status == TaskStatus::Done;
                // optional quality POI
                let quality = self
                    .store
                    .get_poi(&job.workspace, "routing", "quality")
                    .ok()
                    .flatten()
                    .and_then(|p| p.value.as_f64());
                let _ = aff.observe_outcome(&job.workspace, class, mid, success, quality);
            }
        }
        Ok(())
    }

    pub fn report_md(&self, job_id: &str) -> Result<String> {
        let job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| OrqError::Other("job not found".into()))?;
        let children = self.store.job_children(job_id)?;
        let mut md = format!(
            "# Job `{}` — {}\n\n- class: `{}`\n- strategy: `{}`\n- status: `{}`\n- seed: {}\n- epoch: {}\n- reason: {}\n\n## Children\n\n",
            job.id,
            job.name,
            job.class,
            job.strategy.as_str(),
            job.status.as_str(),
            job.seed,
            job.epoch,
            job.route_reason
        );
        for (tid, role, layer) in children {
            let t = self.store.get_task(&tid)?;
            md.push_str(&format!(
                "- L{layer} **{role}** `{}` model={} status={}\n",
                &tid[..8.min(tid.len())],
                t.as_ref()
                    .and_then(|x| x.model_id.clone())
                    .unwrap_or_default(),
                t.as_ref()
                    .map(|x| x.status.as_str())
                    .unwrap_or("?")
            ));
        }
        Ok(md)
    }
}
