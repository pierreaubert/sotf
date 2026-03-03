# Code Review Skill
1. Read all uncommitted changes with `git diff`
2. For each file, identify: correctness bugs, edge cases, missing error handling, algorithm issues
3. Write a failing test for each bug found
4. Fix all identified issues
5. Run `cargo test --workspace` and confirm all tests pass
6. Run `cargo clippy --workspace` and fix any warnings
7. Present summary of findings and fixes
