import json
import jsonschema
from jsonschema import validate

schema_path = "input_schema.json"
config_path = "test_config_2_1.json"

with open(schema_path, 'r') as f:
    schema = json.load(f)

with open(config_path, 'r') as f:
    config = json.load(f)

try:
    validate(instance=config, schema=schema)
    print("Validation successful!")
except jsonschema.exceptions.ValidationError as err:
    print(f"Validation failed: {err}")
    exit(1)
