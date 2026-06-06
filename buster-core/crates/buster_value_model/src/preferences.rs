use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreferenceDomain {
    Music,
    Game,
    Color,
    Topic,
    ResearchField,
    InteractionStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreferenceSignal {
    pub domain: PreferenceDomain,
    pub object: String,
    pub features: Vec<String>,
    pub valence: f32,
    pub intensity: f32,
    pub novelty: f32,
    pub coherence: f32,
    pub evidence_count: u32,
    pub source_type: String,
    pub notes: String,
}

impl PreferenceSignal {
    pub fn new(domain: PreferenceDomain, object: impl Into<String>) -> Self {
        Self {
            domain,
            object: object.into(),
            features: Vec::new(),
            valence: 0.0,
            intensity: 0.0,
            novelty: 0.0,
            coherence: 0.0,
            evidence_count: 1,
            source_type: "unknown".to_string(),
            notes: String::new(),
        }
    }

    pub fn with_features(mut self, features: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.features = features.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_response(mut self, valence: f32, intensity: f32) -> Self {
        self.valence = valence.clamp(-1.0, 1.0);
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    pub fn with_pattern(mut self, novelty: f32, coherence: f32) -> Self {
        self.novelty = novelty.clamp(0.0, 1.0);
        self.coherence = coherence.clamp(0.0, 1.0);
        self
    }

    pub fn with_source(mut self, source_type: impl Into<String>) -> Self {
        self.source_type = source_type.into();
        self
    }
}
