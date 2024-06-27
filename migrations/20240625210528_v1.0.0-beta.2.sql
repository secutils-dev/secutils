-- Rename responder's path column to location.
ALTER TABLE user_data_webhooks_responders RENAME COLUMN path TO location;

-- Migrate all responders to use root subdomain (@) and exact path match (=).
UPDATE user_data_webhooks_responders SET location = CONCAT('@:=:', location);
