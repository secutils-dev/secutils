mod home_summary_get;
mod scheduler_parse_schedule;
mod search;
mod security_subscription_update;
mod security_users_email;
mod security_users_get;
mod security_users_get_by_email;
mod security_users_get_self;
mod security_users_remove;
mod security_users_signup;
mod send_message;
mod status_get;
mod status_set;
mod ui_state_get;
mod user_data_export;
mod user_data_import;
pub mod user_scripts;
pub mod user_secrets;
mod user_settings_get;
mod user_settings_set;
pub mod user_tags;
mod utils_action;
mod webhooks_responders;
mod webhooks_retrack;

pub use self::{
    home_summary_get::home_summary_get,
    scheduler_parse_schedule::scheduler_parse_schedule,
    search::search,
    security_subscription_update::security_subscription_update,
    security_users_email::security_users_email,
    security_users_get::security_users_get,
    security_users_get_by_email::security_users_get_by_email,
    security_users_get_self::security_users_get_self,
    security_users_remove::security_users_remove,
    security_users_signup::security_users_signup,
    send_message::send_message,
    status_get::status_get,
    status_set::status_set,
    ui_state_get::ui_state_get,
    user_data_export::user_data_export,
    user_data_import::{user_data_import, user_data_import_preview},
    user_settings_get::user_settings_get,
    user_settings_set::user_settings_set,
    utils_action::utils_action,
    webhooks_responders::webhooks_responders,
    webhooks_retrack::webhooks_retrack,
};

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Secutils",
        license(
            name = "AGPL-3.0",
            url = "https://github.com/secutils-dev/secutils/blob/main/LICENSE"
        )
    ),
    paths(
        user_tags::user_tags_list,
        user_tags::user_tags_create,
        user_tags::user_tags_update,
        user_tags::user_tags_delete,
        user_secrets::user_secrets_list,
        user_secrets::user_secrets_create,
        user_secrets::user_secrets_update,
        user_secrets::user_secrets_delete,
        user_scripts::user_scripts_list,
        user_scripts::user_scripts_get,
        user_scripts::user_scripts_create,
        user_scripts::user_scripts_update,
        user_scripts::user_scripts_delete,
    ),
    components(schemas(
        crate::users::UserTag,
        crate::users::EntityTag,
        crate::users::TagCreateParams,
        crate::users::TagUpdateParams,
        crate::users::UserSecret,
        crate::users::SecretCreateParams,
        crate::users::SecretUpdateParams,
        crate::users::UserScript,
        crate::users::UserScriptType,
        crate::users::ScriptContext,
        crate::users::ScriptCreateParams,
        crate::users::ScriptUpdateParams,
    ))
)]
pub(super) struct SecutilsOpenApi;

#[cfg(test)]
mod tests {
    use super::SecutilsOpenApi;
    use insta::assert_json_snapshot;
    use utoipa::OpenApi;

    #[test]
    fn openapi_spec_snapshot() {
        let spec: serde_json::Value =
            serde_json::from_str(&SecutilsOpenApi::openapi().to_json().unwrap()).unwrap();
        assert_json_snapshot!(spec, {".info.version" => "[version]"}, @r###"
        {
          "openapi": "3.1.0",
          "info": {
            "title": "Secutils",
            "description": "An open-source, versatile, yet simple security toolbox for engineers and researchers.",
            "contact": {
              "name": "Aleh Zasypkin",
              "email": "dev@secutils.dev"
            },
            "license": {
              "name": "AGPL-3.0",
              "url": "https://github.com/secutils-dev/secutils/blob/main/LICENSE"
            },
            "version": "[version]"
          },
          "paths": {
            "/api/user/scripts": {
              "get": {
                "tags": [
                  "scripts"
                ],
                "summary": "Lists all scripts for the authenticated user, optionally filtered by context.",
                "operationId": "user_scripts_list",
                "parameters": [
                  {
                    "name": "context",
                    "in": "query",
                    "description": "Optional context to filter scripts by compatibility.",
                    "required": false,
                    "schema": {
                      "oneOf": [
                        {
                          "type": "null"
                        },
                        {
                          "$ref": "#/components/schemas/ScriptContext"
                        }
                      ]
                    }
                  }
                ],
                "responses": {
                  "200": {
                    "description": "List of user scripts.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "type": "array",
                          "items": {
                            "$ref": "#/components/schemas/UserScript"
                          }
                        }
                      }
                    }
                  }
                }
              },
              "post": {
                "tags": [
                  "scripts"
                ],
                "summary": "Creates a new script.",
                "operationId": "user_scripts_create",
                "requestBody": {
                  "content": {
                    "application/json": {
                      "schema": {
                        "$ref": "#/components/schemas/ScriptCreateParams"
                      }
                    }
                  },
                  "required": true
                },
                "responses": {
                  "201": {
                    "description": "Script was successfully created.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "$ref": "#/components/schemas/UserScript"
                        }
                      }
                    }
                  },
                  "400": {
                    "description": "Invalid script parameters."
                  }
                }
              }
            },
            "/api/user/scripts/{script_id}": {
              "get": {
                "tags": [
                  "scripts"
                ],
                "summary": "Gets a single script by ID, including its content.",
                "operationId": "user_scripts_get",
                "parameters": [
                  {
                    "name": "script_id",
                    "in": "path",
                    "required": true,
                    "schema": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                ],
                "responses": {
                  "200": {
                    "description": "Script with the specified ID.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "$ref": "#/components/schemas/UserScript"
                        }
                      }
                    }
                  },
                  "404": {
                    "description": "Script not found."
                  }
                }
              },
              "put": {
                "tags": [
                  "scripts"
                ],
                "summary": "Updates an existing script's content.",
                "operationId": "user_scripts_update",
                "parameters": [
                  {
                    "name": "script_id",
                    "in": "path",
                    "required": true,
                    "schema": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                ],
                "requestBody": {
                  "content": {
                    "application/json": {
                      "schema": {
                        "$ref": "#/components/schemas/ScriptUpdateParams"
                      }
                    }
                  },
                  "required": true
                },
                "responses": {
                  "200": {
                    "description": "Script was successfully updated.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "$ref": "#/components/schemas/UserScript"
                        }
                      }
                    }
                  },
                  "404": {
                    "description": "Script not found."
                  }
                }
              },
              "delete": {
                "tags": [
                  "scripts"
                ],
                "summary": "Deletes a script by ID.",
                "operationId": "user_scripts_delete",
                "parameters": [
                  {
                    "name": "script_id",
                    "in": "path",
                    "required": true,
                    "schema": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                ],
                "responses": {
                  "204": {
                    "description": "Script was successfully deleted."
                  },
                  "404": {
                    "description": "Script not found."
                  }
                }
              }
            },
            "/api/user/secrets": {
              "get": {
                "tags": [
                  "secrets"
                ],
                "summary": "Lists all secrets for the authenticated user (metadata only, no values).",
                "operationId": "user_secrets_list",
                "responses": {
                  "200": {
                    "description": "List of user secrets.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "type": "array",
                          "items": {
                            "$ref": "#/components/schemas/UserSecret"
                          }
                        }
                      }
                    }
                  }
                }
              },
              "post": {
                "tags": [
                  "secrets"
                ],
                "summary": "Creates a new secret.",
                "operationId": "user_secrets_create",
                "requestBody": {
                  "content": {
                    "application/json": {
                      "schema": {
                        "$ref": "#/components/schemas/SecretCreateParams"
                      }
                    }
                  },
                  "required": true
                },
                "responses": {
                  "201": {
                    "description": "Secret was successfully created.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "$ref": "#/components/schemas/UserSecret"
                        }
                      }
                    }
                  },
                  "400": {
                    "description": "Invalid secret parameters."
                  }
                }
              }
            },
            "/api/user/secrets/{secret_id}": {
              "put": {
                "tags": [
                  "secrets"
                ],
                "summary": "Updates an existing secret's value.",
                "operationId": "user_secrets_update",
                "parameters": [
                  {
                    "name": "secret_id",
                    "in": "path",
                    "required": true,
                    "schema": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                ],
                "requestBody": {
                  "content": {
                    "application/json": {
                      "schema": {
                        "$ref": "#/components/schemas/SecretUpdateParams"
                      }
                    }
                  },
                  "required": true
                },
                "responses": {
                  "200": {
                    "description": "Secret was successfully updated.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "$ref": "#/components/schemas/UserSecret"
                        }
                      }
                    }
                  },
                  "404": {
                    "description": "Secret not found."
                  }
                }
              },
              "delete": {
                "tags": [
                  "secrets"
                ],
                "summary": "Deletes a secret by ID.",
                "operationId": "user_secrets_delete",
                "parameters": [
                  {
                    "name": "secret_id",
                    "in": "path",
                    "required": true,
                    "schema": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                ],
                "responses": {
                  "204": {
                    "description": "Secret was successfully deleted."
                  },
                  "404": {
                    "description": "Secret not found."
                  }
                }
              }
            },
            "/api/user/tags": {
              "get": {
                "tags": [
                  "tags"
                ],
                "summary": "Lists all tags for the authenticated user.",
                "operationId": "user_tags_list",
                "responses": {
                  "200": {
                    "description": "List of user tags.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "type": "array",
                          "items": {
                            "$ref": "#/components/schemas/UserTag"
                          }
                        }
                      }
                    }
                  }
                }
              },
              "post": {
                "tags": [
                  "tags"
                ],
                "summary": "Creates a new tag.",
                "operationId": "user_tags_create",
                "requestBody": {
                  "content": {
                    "application/json": {
                      "schema": {
                        "$ref": "#/components/schemas/TagCreateParams"
                      }
                    }
                  },
                  "required": true
                },
                "responses": {
                  "201": {
                    "description": "Tag was successfully created.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "$ref": "#/components/schemas/UserTag"
                        }
                      }
                    }
                  },
                  "400": {
                    "description": "Invalid tag parameters."
                  }
                }
              }
            },
            "/api/user/tags/{tag_id}": {
              "put": {
                "tags": [
                  "tags"
                ],
                "summary": "Updates an existing tag's name and/or color.",
                "operationId": "user_tags_update",
                "parameters": [
                  {
                    "name": "tag_id",
                    "in": "path",
                    "required": true,
                    "schema": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                ],
                "requestBody": {
                  "content": {
                    "application/json": {
                      "schema": {
                        "$ref": "#/components/schemas/TagUpdateParams"
                      }
                    }
                  },
                  "required": true
                },
                "responses": {
                  "200": {
                    "description": "Tag was successfully updated.",
                    "content": {
                      "application/json": {
                        "schema": {
                          "$ref": "#/components/schemas/UserTag"
                        }
                      }
                    }
                  },
                  "404": {
                    "description": "Tag not found."
                  }
                }
              },
              "delete": {
                "tags": [
                  "tags"
                ],
                "summary": "Deletes a tag by ID.",
                "operationId": "user_tags_delete",
                "parameters": [
                  {
                    "name": "tag_id",
                    "in": "path",
                    "required": true,
                    "schema": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                ],
                "responses": {
                  "204": {
                    "description": "Tag was successfully deleted."
                  },
                  "404": {
                    "description": "Tag not found."
                  }
                }
              }
            }
          },
          "components": {
            "schemas": {
              "EntityTag": {
                "type": "object",
                "description": "Slim tag representation embedded in entity API responses and entity-level\nexport data. Contains only the fields the UI needs to render a tag badge.",
                "required": [
                  "id",
                  "name",
                  "color"
                ],
                "properties": {
                  "color": {
                    "type": "string"
                  },
                  "id": {
                    "type": "string",
                    "format": "uuid"
                  },
                  "name": {
                    "type": "string"
                  }
                }
              },
              "ScriptContext": {
                "type": "string",
                "description": "Represents the context where a script can be used.",
                "enum": [
                  "responder",
                  "api_tracker",
                  "page_tracker"
                ]
              },
              "ScriptCreateParams": {
                "type": "object",
                "required": [
                  "name",
                  "scriptType",
                  "content"
                ],
                "properties": {
                  "content": {
                    "type": "string"
                  },
                  "name": {
                    "type": "string"
                  },
                  "scriptType": {
                    "type": "string"
                  },
                  "tagIds": {
                    "type": "array",
                    "items": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                },
                "example": {
                  "name": "my-extractor",
                  "scriptType": "api_extractor",
                  "content": "export default async function() { return document.title; }",
                  "tagIds": []
                }
              },
              "ScriptUpdateParams": {
                "type": "object",
                "required": [
                  "content"
                ],
                "properties": {
                  "content": {
                    "type": "string"
                  },
                  "tagIds": {
                    "type": [
                      "array",
                      "null"
                    ],
                    "items": {
                      "type": "string",
                      "format": "uuid"
                    }
                  }
                },
                "example": {
                  "content": "export default async function() { return document.body.innerText; }"
                }
              },
              "SecretCreateParams": {
                "type": "object",
                "required": [
                  "name",
                  "value"
                ],
                "properties": {
                  "name": {
                    "type": "string"
                  },
                  "tagIds": {
                    "type": "array",
                    "items": {
                      "type": "string",
                      "format": "uuid"
                    }
                  },
                  "value": {
                    "type": "string"
                  }
                },
                "example": {
                  "name": "GITHUB_TOKEN",
                  "value": "ghp_xxxxxxxxxxxx",
                  "tagIds": []
                }
              },
              "SecretUpdateParams": {
                "type": "object",
                "required": [
                  "value"
                ],
                "properties": {
                  "tagIds": {
                    "type": [
                      "array",
                      "null"
                    ],
                    "items": {
                      "type": "string",
                      "format": "uuid"
                    }
                  },
                  "value": {
                    "type": "string"
                  }
                },
                "example": {
                  "value": "ghp_yyyyyyyyyyyy"
                }
              },
              "TagCreateParams": {
                "type": "object",
                "required": [
                  "name"
                ],
                "properties": {
                  "color": {
                    "type": "string"
                  },
                  "name": {
                    "type": "string"
                  }
                },
                "example": {
                  "name": "production",
                  "color": "primary"
                }
              },
              "TagUpdateParams": {
                "type": "object",
                "properties": {
                  "color": {
                    "type": [
                      "string",
                      "null"
                    ]
                  },
                  "name": {
                    "type": [
                      "string",
                      "null"
                    ]
                  }
                },
                "example": {
                  "name": "staging",
                  "color": "#54B399"
                }
              },
              "UserScript": {
                "type": "object",
                "description": "Represents a user-defined script for reuse across responders and trackers.",
                "required": [
                  "id",
                  "name",
                  "scriptType",
                  "content",
                  "createdAt",
                  "updatedAt"
                ],
                "properties": {
                  "content": {
                    "type": "string",
                    "description": "The script content (the actual code)."
                  },
                  "createdAt": {
                    "type": "integer",
                    "format": "int64",
                    "description": "When the script was first created."
                  },
                  "id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "Unique identifier for the script."
                  },
                  "name": {
                    "type": "string",
                    "description": "The script name (used to reference it in the UI)."
                  },
                  "scriptType": {
                    "$ref": "#/components/schemas/UserScriptType",
                    "description": "The type of script, determining compatible contexts."
                  },
                  "tags": {
                    "type": "array",
                    "items": {
                      "$ref": "#/components/schemas/EntityTag"
                    },
                    "description": "Tags assigned to this script."
                  },
                  "updatedAt": {
                    "type": "integer",
                    "format": "int64",
                    "description": "When the script content was last updated."
                  }
                }
              },
              "UserScriptType": {
                "type": "string",
                "description": "Represents the type of user script, determining where it can be used.",
                "enum": [
                  "responder",
                  "api_configurator",
                  "api_extractor",
                  "page_extractor",
                  "universal"
                ]
              },
              "UserSecret": {
                "type": "object",
                "description": "Represents a user secret (key-value pair stored encrypted at rest).\nThe value is never returned to clients after creation.",
                "required": [
                  "id",
                  "name",
                  "createdAt",
                  "updatedAt"
                ],
                "properties": {
                  "createdAt": {
                    "type": "integer",
                    "format": "int64",
                    "description": "When the secret was first created."
                  },
                  "id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "Unique identifier for the secret."
                  },
                  "name": {
                    "type": "string",
                    "description": "The secret name (used to reference it in scripts and templates)."
                  },
                  "tags": {
                    "type": "array",
                    "items": {
                      "$ref": "#/components/schemas/EntityTag"
                    },
                    "description": "Tags assigned to this secret."
                  },
                  "updatedAt": {
                    "type": "integer",
                    "format": "int64",
                    "description": "When the secret value was last updated."
                  }
                }
              },
              "UserTag": {
                "type": "object",
                "description": "A user-managed tag with a name and display color.",
                "required": [
                  "id",
                  "name",
                  "color",
                  "createdAt",
                  "updatedAt"
                ],
                "properties": {
                  "color": {
                    "type": "string"
                  },
                  "createdAt": {
                    "type": "integer",
                    "format": "int64"
                  },
                  "id": {
                    "type": "string",
                    "format": "uuid"
                  },
                  "name": {
                    "type": "string"
                  },
                  "updatedAt": {
                    "type": "integer",
                    "format": "int64"
                  }
                }
              }
            }
          }
        }
        "###);
    }
}
