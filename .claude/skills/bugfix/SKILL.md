# Bug Fix Workflow
1. Read the user's bug description carefully
2. Investigate the FULL signal chain / call path — do not stop at the first symptom
3. Identify the root cause, not just the trigger
4. Implement the fix across all affected files
5. Run `cargo test` and ensure ALL tests pass
6. Run `cargo clippy -- -W warnings` and fix any new warnings
7. Summarize: root cause, what changed, files modified, test results
