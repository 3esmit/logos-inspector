use crate::{
    TransactionSummary,
    program_decode::{
        ProgramDecodeCandidate, ResolvedAccountDecodeSession, ResolvedTransactionDecodeSession,
        resolve_account_decode_session, resolve_transaction_decode_session,
    },
    support::entity_id::normalize_program_id_hex,
};

pub(crate) fn select_account_decode_session(
    account_id: Option<&str>,
    owner_program_id_hex: Option<&str>,
    data_hex: &str,
    candidates: &[ProgramDecodeCandidate],
) -> ResolvedAccountDecodeSession {
    let ordered = ordered_account_candidates(owner_program_id_hex, candidates);
    resolve_account_decode_session(account_id, data_hex, &ordered)
}

pub(crate) fn select_transaction_decode_session(
    summary: &TransactionSummary,
    candidates: &[ProgramDecodeCandidate],
) -> ResolvedTransactionDecodeSession {
    let ordered = ordered_transaction_candidates(summary, candidates);
    resolve_transaction_decode_session(summary, &ordered)
}

fn ordered_account_candidates(
    owner_program_id_hex: Option<&str>,
    candidates: &[ProgramDecodeCandidate],
) -> Vec<ProgramDecodeCandidate> {
    let owner = normalized_program_id(owner_program_id_hex);
    let mut rows = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate_matches_owner(candidate, owner.as_deref()))
        .map(|(index, candidate)| (account_candidate_rank(candidate), index, candidate.clone()))
        .collect::<Vec<_>>();
    rows.sort_by_key(|(rank, index, _)| (*rank, *index));
    rows.into_iter()
        .map(|(_, _, candidate)| candidate)
        .collect()
}

fn ordered_transaction_candidates(
    summary: &TransactionSummary,
    candidates: &[ProgramDecodeCandidate],
) -> Vec<ProgramDecodeCandidate> {
    let program = normalized_program_id(summary.program_id_hex.as_deref());
    let mut rows = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate_matches_owner(candidate, program.as_deref()))
        .map(|(index, candidate)| {
            (
                transaction_candidate_rank(candidate),
                index,
                candidate.clone(),
            )
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|(rank, index, _)| (*rank, *index));
    rows.into_iter()
        .map(|(_, _, candidate)| candidate)
        .collect()
}

fn account_candidate_rank(candidate: &ProgramDecodeCandidate) -> u8 {
    match (candidate.cached, candidate.shared, candidate.owner_matched) {
        (true, false, _) => 0,
        (_, false, true) => 1,
        (true, true, _) => 2,
        (_, true, _) => 3,
        _ => 4,
    }
}

fn transaction_candidate_rank(candidate: &ProgramDecodeCandidate) -> u8 {
    if candidate.cached { 0 } else { 1 }
}

fn candidate_matches_owner(candidate: &ProgramDecodeCandidate, owner: Option<&str>) -> bool {
    let Some(owner) = owner.filter(|value| !value.is_empty()) else {
        return true;
    };
    normalized_program_id(Some(candidate.program_id_hex.as_str()))
        .as_deref()
        .is_some_and(|program_id| program_id == owner)
}

fn normalized_program_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| normalize_program_id_hex(value).ok())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        key: &str,
        program_id_hex: &str,
        cached: bool,
        shared: bool,
        owner_matched: bool,
    ) -> ProgramDecodeCandidate {
        ProgramDecodeCandidate {
            key: key.to_owned(),
            name: key.to_owned(),
            program_id_hex: program_id_hex.to_owned(),
            json: "{}".to_owned(),
            account_type: None,
            source: None,
            cached,
            shared,
            owner_matched,
        }
    }

    #[test]
    fn account_selection_orders_cached_owner_and_shared_candidates() {
        let program = "0100000000000000000000000000000000000000000000000000000000000000";
        let candidates = [
            candidate("shared", program, false, true, false),
            candidate("owner", program, false, false, true),
            candidate("cached-shared", program, true, true, false),
            candidate("cached-local", program, true, false, false),
        ];

        let ordered = ordered_account_candidates(Some(program), &candidates);
        let keys = ordered
            .iter()
            .map(|candidate| candidate.key.as_str())
            .collect::<Vec<_>>();

        assert_eq!(keys, ["cached-local", "owner", "cached-shared", "shared"]);
    }

    #[test]
    fn account_selection_filters_owner_mismatched_candidates() {
        let program = "0100000000000000000000000000000000000000000000000000000000000000";
        let other = "0200000000000000000000000000000000000000000000000000000000000000";
        let candidates = [
            candidate("matching", program, false, false, true),
            candidate("mismatched", other, true, false, false),
        ];

        let ordered = ordered_account_candidates(Some(program), &candidates);
        let keys = ordered
            .iter()
            .map(|candidate| candidate.key.as_str())
            .collect::<Vec<_>>();

        assert_eq!(keys, ["matching"]);
    }

    #[test]
    fn transaction_selection_prefers_cached_account_bound_candidate() {
        let program = "0100000000000000000000000000000000000000000000000000000000000000";
        let other = "0200000000000000000000000000000000000000000000000000000000000000";
        let summary = TransactionSummary {
            hash: "tx".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some(program.to_owned()),
            account_ids: Vec::new(),
            nonces: Vec::new(),
            instruction_data: Vec::new(),
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };
        let candidates = [
            candidate("program", program, false, false, false),
            candidate("cached-account", program, true, false, false),
            candidate("other-program", other, true, false, false),
        ];

        let ordered = ordered_transaction_candidates(&summary, &candidates);
        let keys = ordered
            .iter()
            .map(|candidate| candidate.key.as_str())
            .collect::<Vec<_>>();

        assert_eq!(keys, ["cached-account", "program"]);
    }
}
