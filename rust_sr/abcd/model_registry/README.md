# Model Registry

This folder is for hand-promoted model artifacts that should survive cleanup of
temporary logs and training output.

Each model folder should include:

- `model.cbm`: the exact CatBoost model artifact.
- `metadata.json`: run id, source data, training settings, and artifact hash.

Temporary training output still belongs in `tmp_logs/` and should remain ignored.
