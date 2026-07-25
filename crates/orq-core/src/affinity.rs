use crate::error::Result;
use crate::store::Store;
use crate::types::*;

const EMA_ALPHA: f64 = 0.3;
const SUCCESS_REWARD: f64 = 1.0;
const FAIL_PENALTY: f64 = 0.0;

pub struct AffinityEngine<'a> {
    store: &'a Store,
}

impl<'a> AffinityEngine<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn set(
        &self,
        workspace: &str,
        class: &str,
        model_id: &str,
        score: f64,
    ) -> Result<AffinityScore> {
        let epoch = self.store.get_affinity_epoch(workspace)?;
        let a = AffinityScore {
            workspace: workspace.into(),
            class: class.into(),
            model_id: model_id.into(),
            score: score.clamp(0.0, 1.0),
            confidence: 0.5,
            n: 0,
            epoch,
            updated_at: now(),
        };
        self.store.upsert_affinity(&a)?;
        Ok(a)
    }

    pub fn list(&self, workspace: &str, class: Option<&str>) -> Result<Vec<AffinityScore>> {
        self.store.list_affinities(workspace, class)
    }

    pub fn bump_epoch(&self, workspace: &str) -> Result<i64> {
        self.store.bump_affinity_epoch(workspace)
    }

    pub fn epoch(&self, workspace: &str) -> Result<i64> {
        self.store.get_affinity_epoch(workspace)
    }

    /// Ensure every registered model has an affinity row for class (default 0.5).
    pub fn ensure_defaults(&self, workspace: &str, class: &str) -> Result<Vec<AffinityScore>> {
        let epoch = self.store.get_affinity_epoch(workspace)?;
        let models = self.store.list_models(workspace)?;
        let mut out = Vec::new();
        for m in models {
            if let Some(a) = self.store.get_affinity(workspace, class, &m.id)? {
                out.push(a);
            } else {
                let a = AffinityScore {
                    workspace: workspace.into(),
                    class: class.into(),
                    model_id: m.id,
                    score: 0.5,
                    confidence: 0.0,
                    n: 0,
                    epoch,
                    updated_at: now(),
                };
                self.store.upsert_affinity(&a)?;
                out.push(a);
            }
        }
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.model_id.cmp(&b.model_id))
        });
        Ok(out)
    }

    pub fn observe_outcome(
        &self,
        workspace: &str,
        class: &str,
        model_id: &str,
        success: bool,
        quality: Option<f64>,
    ) -> Result<AffinityScore> {
        let epoch = self.store.get_affinity_epoch(workspace)?;
        let mut a = self
            .store
            .get_affinity(workspace, class, model_id)?
            .unwrap_or(AffinityScore {
                workspace: workspace.into(),
                class: class.into(),
                model_id: model_id.into(),
                score: 0.5,
                confidence: 0.0,
                n: 0,
                epoch,
                updated_at: now(),
            });
        let reward = quality
            .map(|q| q.clamp(0.0, 1.0))
            .unwrap_or(if success {
                SUCCESS_REWARD
            } else {
                FAIL_PENALTY
            });
        a.score = (EMA_ALPHA * reward + (1.0 - EMA_ALPHA) * a.score).clamp(0.0, 1.0);
        a.n += 1;
        a.confidence = (a.n as f64 / (a.n as f64 + 5.0)).clamp(0.0, 1.0);
        a.epoch = epoch;
        a.updated_at = now();
        self.store.upsert_affinity(&a)?;
        Ok(a)
    }
}

/// Stable u64 seed from workspace/class/epoch (+ optional override).
pub fn derive_seed(workspace: &str, class: &str, epoch: i64, override_seed: Option<u64>) -> u64 {
    if let Some(s) = override_seed {
        return s;
    }
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    workspace.hash(&mut h);
    class.hash(&mut h);
    epoch.hash(&mut h);
    h.finish()
}

/// Seeded xorshift64* RNG for exploration.
pub struct SeedRng {
    state: u64,
}

impl SeedRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e3779b97f4a7c15 } else { seed },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }

    pub fn gen_range(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        (self.next_u64() as usize) % max
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelRegistry;
    use tempfile::tempdir;

    #[test]
    fn ema_and_seed_stable() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.ensure_workspace("default", None).unwrap();
        ModelRegistry::new(&store)
            .add_cli("default", "a", "echo a {cmd}", vec!["code".into()])
            .unwrap();
        ModelRegistry::new(&store)
            .add_cli("default", "b", "echo b {cmd}", vec!["code".into()])
            .unwrap();
        let eng = AffinityEngine::new(&store);
        eng.ensure_defaults("default", "code.edit").unwrap();
        eng.observe_outcome("default", "code.edit", "a", true, None)
            .unwrap();
        let a = store.get_affinity("default", "code.edit", "a").unwrap().unwrap();
        assert!(a.score > 0.5);
        let s1 = derive_seed("default", "code.edit", 0, None);
        let s2 = derive_seed("default", "code.edit", 0, None);
        assert_eq!(s1, s2);
    }
}
