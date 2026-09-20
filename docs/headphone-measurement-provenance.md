# Headphone measurement identity

Headphone EQ file import reads AutoEQ's optional versioned provenance sidecar.
For `measurement.csv`, the sidecar is `measurement.csv.provenance.json`.
Use AutoEQ's `MeasurementRecord` and `write_sidecar` API to produce the complete
record, including the curve content hash.

The measurement identity disclosure displays these optional string fields from
`provenance.acquisition.extensions`:

| Field | Display |
|---|---|
| `model` | Model or variant |
| `rig` | Measurement rig |
| `sample` | Sample, unit, ear or measurement-position description, alongside rig |
| `compensation` | Source-supplied compensation description |

The complete record is retained with the imported preview. Empty, missing and
non-string identity fields display as not supplied. Selecting a target does not
rewrite measurement metadata. Importing a replacement without a sidecar clears
the previous file's identity.

The shared importer validates the sidecar using AutoEQ's schema and size limits,
checks its internal curve hash, and checks that its hash matches the CSV curve.
An invalid or mismatched sidecar fails the import; the previously accepted
measurement remains selected. Files without sidecars continue to work.

Hash validation establishes that the record describes the imported curve; it
does not certify the source's rig description or target compatibility. The UI
continues to ask the listener to confirm that compatibility. Catalog downloads
currently do not provide this metadata through the upstream download API.
