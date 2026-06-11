# ----------------------------------------------------------------------
# DOC
# ----------------------------------------------------------------------

[group('doc')]
doc-cargo:
	cargo doc --all --no-deps

[group('doc')]
doc-crates:
	cargo run -p sotf-docs-gen -- --check

# Build the Texinfo manual (sotf.info, sotf.html, sotf.pdf).
# Requires `makeinfo` (texinfo) and, for PDF, `texi2pdf` / TeX Live.
[group('doc')]
doc-info:
	makeinfo --no-split docs/sotf.texi -o docs/sotf.info

[group('doc')]
doc-html:
	makeinfo --html --no-split docs/sotf.texi -o docs/sotf.html

[group('doc')]
doc-pdf:
	cd docs && texi2pdf --clean sotf.texi

[group('doc')]
doc-manual: doc-info doc-html

[group('doc')]
doc: doc-cargo doc-crates doc-manual



