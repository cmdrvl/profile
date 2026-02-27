use serde_json::{Value, json};

/// Generate JSON Schema for the Profile YAML format
pub fn generate_profile_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://epistemic.so/schemas/profile.v1.json",
        "title": "Profile Schema",
        "description": "Schema for column-scoping profiles used by report tools",
        "type": "object",
        "required": [
            "schema_version",
            "status",
            "format",
            "include_columns"
        ],
        "properties": {
            "schema_version": {
                "type": "integer",
                "const": 1,
                "description": "Schema version (must be 1)"
            },
            "profile_id": {
                "type": "string",
                "pattern": "^[a-z0-9]+(?:\\.[a-z0-9_]+)*\\.v[0-9]+$",
                "description": "Immutable profile identifier (<family>.v<version>), required for frozen profiles"
            },
            "profile_version": {
                "type": "integer",
                "minimum": 1,
                "description": "Profile version number, required for frozen profiles"
            },
            "profile_family": {
                "type": "string",
                "pattern": "^[a-z0-9]+(?:\\.[a-z0-9_]+)*$",
                "description": "Profile family name, required for frozen profiles"
            },
            "profile_sha256": {
                "type": "string",
                "pattern": "^sha256:[a-f0-9]{64}$",
                "description": "SHA256 hash of canonical profile, required for frozen profiles"
            },
            "status": {
                "type": "string",
                "enum": ["draft", "frozen"],
                "description": "Profile status"
            },
            "format": {
                "type": "string",
                "enum": ["csv"],
                "description": "Dataset format (only CSV supported in v0.1)"
            },
            "hashing": {
                "type": "object",
                "properties": {
                    "algorithm": {
                        "type": "string",
                        "enum": ["sha256"],
                        "description": "Hashing algorithm"
                    }
                },
                "required": ["algorithm"],
                "additionalProperties": false
            },
            "equivalence": {
                "type": "object",
                "properties": {
                    "order": {
                        "type": "string",
                        "enum": ["order-invariant", "order-sensitive"],
                        "description": "Row equivalence ordering"
                    },
                    "float_decimals": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 15,
                        "description": "Float precision for equivalence"
                    },
                    "trim_strings": {
                        "type": "boolean",
                        "description": "Whether to trim strings for equivalence"
                    }
                },
                "additionalProperties": false
            },
            "key": {
                "type": "array",
                "items": {
                    "type": "string",
                    "minLength": 1
                },
                "description": "Key column names for deduplication"
            },
            "include_columns": {
                "type": "array",
                "items": {
                    "type": "string",
                    "minLength": 1
                },
                "minItems": 1,
                "description": "Column names to include in analysis (required for frozen profiles)"
            }
        },
        "additionalProperties": false,
        "allOf": [
            {
                "if": {
                    "properties": {
                        "status": {"const": "frozen"}
                    }
                },
                "then": {
                    "required": [
                        "profile_id",
                        "profile_version",
                        "profile_family",
                        "profile_sha256"
                    ],
                    "properties": {
                        "include_columns": {
                            "minItems": 1
                        }
                    }
                }
            },
            {
                "if": {
                    "properties": {
                        "status": {"const": "draft"}
                    }
                },
                "then": {
                    "not": {
                        "anyOf": [
                            {"required": ["profile_id"]},
                            {"required": ["profile_version"]},
                            {"required": ["profile_family"]},
                            {"required": ["profile_sha256"]}
                        ]
                    }
                }
            }
        ]
    })
}
