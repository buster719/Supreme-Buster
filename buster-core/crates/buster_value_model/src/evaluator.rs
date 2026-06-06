use crate::types::{ActionCandidate, ValueContext, ValueJudgement};

pub trait ValueEvaluator {
    fn evaluate(&self, context: &ValueContext, candidate: &ActionCandidate) -> ValueJudgement;
}
