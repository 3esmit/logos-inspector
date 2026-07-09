mod rulebook;

pub(crate) use rulebook::source_facts_for_report;
pub use rulebook::{
    SourceCapabilityFact, SourceFacts, SourceHealthFacts, SourceHealthStatus, SourceProbeFact,
};
