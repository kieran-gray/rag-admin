ALTER TABLE pipeline_configurations
ADD COLUMN is_default BOOLEAN NOT NULL DEFAULT FALSE;

CREATE UNIQUE INDEX pipeline_configurations_one_default
ON pipeline_configurations (is_default)
WHERE is_default;

ALTER TABLE chunking_configurations
ADD COLUMN is_default BOOLEAN NOT NULL DEFAULT FALSE;

CREATE UNIQUE INDEX chunking_configurations_one_default
ON chunking_configurations (is_default)
WHERE is_default;
