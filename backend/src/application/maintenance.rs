//! One bounded observation followed by at most one fresh-authority mutation.
use crate::persistence::*;

#[derive(Debug)]
pub enum MaintenanceOutcome {
    EndOfTraversal,
    Observed {
        cursor: DiscoveryCursor,
        evidence: CandidateEvidence,
    },
    Isolation(Result<RetryTransition<PreservedFailure>, PersistenceError>),
}
/// Advance before mutation; no repeat of a consumed observation after errors,
/// conflicts or uncertain commits. A future traversal obtains fresh evidence.
pub async fn maintain_once<R>(
    repository: &R,
    cursor: &mut DiscoveryCursor,
) -> Result<MaintenanceOutcome, PersistenceError>
where
    R: OutboxCandidateDiscovery + OutboxCandidateIsolator,
{
    let Some(observation) = repository.discover_candidate(*cursor).await? else {
        *cursor = DiscoveryCursor::default();
        return Ok(MaintenanceOutcome::EndOfTraversal);
    };
    *cursor = observation.cursor;
    Ok(match observation.candidate {
        Some(candidate)
            if candidate.evidence == observation.evidence
                && candidate.evidence.terminal_reason().is_some() =>
        {
            MaintenanceOutcome::Isolation(repository.isolate_candidate(candidate).await)
        }
        _ => MaintenanceOutcome::Observed {
            cursor: observation.cursor,
            evidence: observation.evidence,
        },
    })
}
