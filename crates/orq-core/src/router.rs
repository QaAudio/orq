use crate::affinity::{derive_seed, AffinityEngine, SeedRng};
use crate::error::{OrqError, Result};
use crate::store::Store;
use crate::types::*;

const DEFAULT_EPSILON: f64 = 0.1;
const DEFAULT_TEMPERATURE: f64 = 0.7;

pub struct Router<'a> {
    store: &'a Store,
}

impl<'a> Router<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn decide(
        &self,
        workspace: &str,
        class: &str,
        policy: RoutePolicy,
        k: u32,
        seed_override: Option<u64>,
        capability_filter: Option<&str>,
        aggregator_hint: Option<&str>,
        force_model: Option<&str>,
    ) -> Result<RouteDecision> {
        let epoch = self.store.get_affinity_epoch(workspace)?;
        let seed = derive_seed(workspace, class, epoch, seed_override);
        let aff = AffinityEngine::new(self.store);
        let mut scores = aff.ensure_defaults(workspace, class)?;

        if scores.is_empty() {
            return Err(OrqError::Other(
                "no models registered; use `orq model add`".into(),
            ));
        }

        if let Some(cap) = capability_filter {
            let models = self.store.list_models(workspace)?;
            let allowed: std::collections::HashSet<_> = models
                .into_iter()
                .filter(|m| m.capabilities.iter().any(|c| c == cap))
                .map(|m| m.id)
                .collect();
            scores.retain(|a| allowed.contains(&a.model_id));
            if scores.is_empty() {
                return Err(OrqError::Other(format!(
                    "no models with capability `{cap}`"
                )));
            }
        }

        // Adjust ranking by cost/latency priors (light touch)
        let models = self.store.list_models(workspace)?;
        let mut ranked: Vec<(AffinityScore, f64)> = scores
            .into_iter()
            .map(|a| {
                let m = models.iter().find(|m| m.id == a.model_id);
                let prior = m
                    .map(|m| 1.0 / (1.0 + m.cost_weight * 0.1 + m.latency_weight * 0.05))
                    .unwrap_or(1.0);
                let adj = a.score * prior;
                (a, adj)
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.model_id.cmp(&b.0.model_id))
        });

        let mut rng = SeedRng::new(seed);
        let k = k.max(1) as usize;

        let picked = if let Some(fm) = force_model {
            if !ranked.iter().any(|(a, _)| a.model_id == fm) {
                return Err(OrqError::Other(format!("forced model not available: {fm}")));
            }
            vec![fm.to_string()]
        } else {
            match policy {
                RoutePolicy::Sticky => ranked
                    .iter()
                    .take(k)
                    .map(|(a, _)| a.model_id.clone())
                    .collect(),
                RoutePolicy::Epsilon => {
                    let mut out = Vec::new();
                    for i in 0..k.min(ranked.len()) {
                        if rng.next_f64() < DEFAULT_EPSILON {
                            let idx = rng.gen_range(ranked.len());
                            out.push(ranked[idx].0.model_id.clone());
                        } else {
                            out.push(ranked[i].0.model_id.clone());
                        }
                    }
                    dedup_preserve(out)
                }
                RoutePolicy::Softmax => {
                    let temps: Vec<f64> = ranked
                        .iter()
                        .map(|(_, adj)| (*adj / DEFAULT_TEMPERATURE).exp())
                        .collect();
                    let sum: f64 = temps.iter().sum::<f64>().max(1e-9);
                    let mut out = Vec::new();
                    for _ in 0..k.min(ranked.len()) {
                        let mut r = rng.next_f64() * sum;
                        let mut chosen = ranked[0].0.model_id.clone();
                        for (i, (a, _)) in ranked.iter().enumerate() {
                            r -= temps[i];
                            if r <= 0.0 {
                                chosen = a.model_id.clone();
                                break;
                            }
                        }
                        out.push(chosen);
                    }
                    dedup_preserve(out)
                }
            }
        };

        let aggregator = aggregator_hint.map(|s| s.to_string()).or_else(|| {
            // default aggregator = highest ranked that isn't only pick, or first
            ranked.first().map(|(a, _)| a.model_id.clone())
        });

        let reason = format!(
            "policy={} epoch={} seed={} top={}",
            policy.as_str(),
            epoch,
            seed,
            picked.join(",")
        );

        let decision = RouteDecision {
            class: class.into(),
            models: picked,
            aggregator,
            policy,
            seed,
            epoch,
            reason: reason.clone(),
        };

        self.store.append_event(
            workspace,
            None,
            "route.decided",
            serde_json::json!({
                "class": class,
                "models": decision.models,
                "aggregator": decision.aggregator,
                "policy": policy.as_str(),
                "seed": seed,
                "epoch": epoch,
                "reason": reason,
            }),
        )?;

        Ok(decision)
    }
}

fn dedup_preserve(items: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for i in items {
        if seen.insert(i.clone()) {
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelRegistry;
    use tempfile::tempdir;

    #[test]
    fn sticky_is_deterministic() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.ensure_workspace("default", None).unwrap();
        let reg = ModelRegistry::new(&store);
        reg.add_cli("default", "m1", "echo m1 {cmd}", vec!["code".into()])
            .unwrap();
        reg.add_cli("default", "m2", "echo m2 {cmd}", vec!["code".into()])
            .unwrap();
        AffinityEngine::new(&store)
            .set("default", "code.edit", "m2", 0.9)
            .unwrap();
        AffinityEngine::new(&store)
            .set("default", "code.edit", "m1", 0.2)
            .unwrap();
        let r = Router::new(&store);
        let d1 = r
            .decide(
                "default",
                "code.edit",
                RoutePolicy::Sticky,
                1,
                Some(42),
                None,
                None,
                None,
            )
            .unwrap();
        let d2 = r
            .decide(
                "default",
                "code.edit",
                RoutePolicy::Sticky,
                1,
                Some(42),
                None,
                None,
                None,
            )
            .unwrap();
        assert_eq!(d1.models, d2.models);
        assert_eq!(d1.models[0], "m2");
    }
}
