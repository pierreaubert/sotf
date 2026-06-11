# ----------------------------------------------------------------------
# UPDATE
# ----------------------------------------------------------------------

[group('install')]
update: update-rust update-pre-commit

[group('install')]
update-rust:
	rustup update
	cargo update

[group('install')]
update-pre-commit:
	pre-commit autoupdate

