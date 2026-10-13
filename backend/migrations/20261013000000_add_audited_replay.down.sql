-- Destructive audit removal; disposable databases only, never uncertainty recovery.
DROP TABLE relay.replay_requests;
DROP FUNCTION relay.reject_replay_audit_change();
