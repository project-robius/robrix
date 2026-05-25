spec: task
name: "Typed Restore Session Error Policy"
inherits: project
tags: [bugfix, login, persistence, matrix, restore-session]
---

## Intent

Make startup auto-login recovery non-destructive unless Robrix has evidence that the saved session is unrecoverable. Today every `restore_session` error deletes both the saved session and `latest_user_id.txt`, so transient homeserver, proxy, Docker, Palpo, endpoint, or Matrix SDK client-build failures become permanent auto-login loss. This task introduces typed restore errors and applies explicit cleanup rules so Robrix keeps retryable sessions while still removing stale pointers, corrupt local files, and confirmed invalid tokens.

## Constraints

- Do not run `cargo fmt` or `rustfmt`.
- Do not add new dependencies.
- Do not change explicit logout cleanup behavior.
- Do not require Palpo to already implement `GET /_matrix/client/v3/account/whoami`; Robrix must treat a `404` from `whoami` as server/endpoint failure, not as token revocation.
- Do not classify arbitrary HTTP 4xx/5xx responses as invalid tokens. Only `M_UNKNOWN_TOKEN`, `M_MISSING_TOKEN`, or SDK `SessionChange::UnknownToken` prove token invalidity.
- Do not print or log access tokens, refresh tokens, or session passphrases.

## Decisions

- `persistence::restore_session` returns a typed error enum instead of `anyhow::Error`.
- The typed enum distinguishes at least these categories: no latest user, missing session file, unreadable session file, corrupt session JSON, Matrix client build failure, SDK restore failure, invalid token, and failure to write `latest_user_id.txt`.
- Startup restore failure policy lives in `src/sliding_sync.rs`; filesystem classification and archive helpers live in `src/persistence/matrix_state.rs`.
- `MissingSessionFile` deletes only `latest_user_id.txt` when it points at the missing session user.
- `CorruptSessionFile` renames the session file to a `.bad` archive path and deletes `latest_user_id.txt` when it points at that user.
- `ClientBuild`, generic SDK restore errors, non-token `whoami` failures, and sync-service non-token failures preserve the session file, Matrix store, and `latest_user_id.txt`.
- Startup restore must not require a proactive `whoami()` validation request to succeed; invalid tokens are still handled by SDK restore, `SyncService::build()`, or `SessionChange::UnknownToken`.
- Confirmed token invalidation still calls the existing `clear_persisted_session(...)` path.
- UI messaging for retryable restore failures must tell the user that Robrix can retry on the next startup after the server or network issue is fixed.

## Boundaries

### Allowed Changes

- src/persistence/matrix_state.rs
- src/persistence/mod.rs
- src/sliding_sync.rs
- specs/task-restore-session-error-policy.spec.md

### Forbidden

- Cargo.toml
- Cargo.lock
- src/logout/**
- Any Palpo or deployment source under `palpo-and-octos-deploy/**`
- Running `cargo fmt` or `rustfmt`
- Replacing the Matrix SDK restore flow with raw HTTP requests

## Out of Scope

- Implementing Palpo's `whoami` endpoint.
- Changing Matrix SDK source code.
- Changing explicit logout, account deletion, or user-initiated session reset behavior.
- Reworking account switching beyond applying the same non-destructive restore policy where it calls `restore_session`.
- Adding telemetry or remote diagnostics.

## Completion Criteria

Scenario: Startup client-build failure preserves auto-login data
  Test: restore_session_policy_preserves_data_for_client_build_failure
  Level: unit
  Test Double: construct a typed `RestoreSessionError::ClientBuild` or equivalent policy input; do not contact a real homeserver
  Given a saved session and `latest_user_id.txt` for "@alice:example.org"
  When startup restore fails with a Matrix client build or homeserver discovery error
  Then Robrix does not delete the session file
  And Robrix does not delete the Matrix store directory
  And Robrix does not delete `latest_user_id.txt`
  And the UI message says the user can retry after fixing server or network availability

Scenario: Retryable whoami validation failure does not revoke the local session
  Test: whoami_404_is_retryable_restore_validation_failure
  Level: unit
  Test Double: fake validation error carrying HTTP 404 without `M_UNKNOWN_TOKEN` or `M_MISSING_TOKEN`
  Given a restored client whose `whoami()` validation returns a retryable non-token error such as timeout or HTTP 404
  When startup validates the restored session
  Then Robrix treats the error as retryable server or endpoint failure
  And Robrix does not call `clear_persisted_session`
  And Robrix does not delete `latest_user_id.txt`

Scenario: Missing session file removes only the stale latest-user pointer
  Test: restore_session_policy_clears_latest_user_for_missing_session_file
  Level: filesystem unit
  Test Double: temporary app data directory or injectable path rooted under a test-only directory
  Given `latest_user_id.txt` points to "@alice:example.org"
  And the session file for "@alice:example.org" does not exist
  When startup restore classifies the error as `MissingSessionFile`
  Then Robrix deletes `latest_user_id.txt`
  And Robrix does not attempt to delete a Matrix store path recovered from the missing session file

Scenario: Corrupt session JSON is archived before latest-user pointer removal
  Test: corrupt_session_file_is_archived_and_latest_user_is_cleared
  Level: filesystem unit
  Test Double: temporary app data directory or injectable path rooted under a test-only directory
  Given `latest_user_id.txt` points to "@alice:example.org"
  And the session file for "@alice:example.org" contains invalid JSON
  When startup restore classifies the error as `CorruptSessionFile`
  Then Robrix renames the original session file to a `.bad` archive path
  And Robrix deletes `latest_user_id.txt`
  And Robrix does not delete the Matrix store directory

Scenario: Confirmed invalid token still clears persisted session
  Test: invalid_token_restore_policy_clears_session_and_latest_user
  Level: unit
  Test Double: fake validation, sync-service, or session-change error carrying `M_UNKNOWN_TOKEN` or `M_MISSING_TOKEN`
  Given a saved session and `latest_user_id.txt` for "@alice:example.org"
  When `whoami()`, `SyncService::build()`, or `SessionChange::UnknownToken` reports `M_UNKNOWN_TOKEN` or `M_MISSING_TOKEN`
  Then Robrix calls `clear_persisted_session` for "@alice:example.org"
  And the session file is deleted
  And `latest_user_id.txt` is deleted if it points at "@alice:example.org"
  And the user is asked to log in again

Scenario: Account-switch restore failure is non-destructive for retryable errors
  Test: account_switch_restore_retryable_error_preserves_target_session
  Level: unit
  Test Double: construct a typed retryable restore error for the target account; do not contact a real homeserver
  Given the user switches to an account with a saved session
  When `restore_session(Some(target_user_id))` fails with `ClientBuild` or generic SDK restore failure
  Then Robrix reports account switch failure
  And Robrix preserves the target account session file
  And Robrix preserves the target account Matrix store directory

Scenario: Save-latest failure after successful SDK restore does not delete the session
  Test: save_latest_user_failure_is_reported_without_session_cleanup
  Level: unit
  Test Double: construct a typed `RestoreSessionError::SaveLatestUserId` or equivalent policy input
  Given SDK restore succeeds for "@alice:example.org"
  When writing `latest_user_id.txt` fails
  Then `restore_session` returns a typed `SaveLatestUserId` error
  And Robrix does not delete the session file
  And Robrix reports a restore failure that includes the latest-user persistence problem without exposing secrets
