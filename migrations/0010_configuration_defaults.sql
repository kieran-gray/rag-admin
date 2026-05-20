CREATE TABLE configuration_defaults (
    id INT PRIMARY KEY DEFAULT 1,
    chunking_configuration_id UUID,
    pipeline_configuration_id UUID,
    sweep_template_id UUID,
    CONSTRAINT configuration_defaults_singleton CHECK (id = 1)
);

INSERT INTO configuration_defaults (id, chunking_configuration_id, pipeline_configuration_id, sweep_template_id)
VALUES (
    1,
    (SELECT id FROM chunking_configurations WHERE is_default = TRUE LIMIT 1),
    (SELECT id FROM pipeline_configurations WHERE is_default = TRUE LIMIT 1),
    (SELECT id FROM sweep_templates WHERE is_default = TRUE LIMIT 1)
);

DROP INDEX IF EXISTS chunking_configurations_one_default;
DROP INDEX IF EXISTS pipeline_configurations_one_default;

ALTER TABLE chunking_configurations DROP COLUMN is_default;
ALTER TABLE pipeline_configurations DROP COLUMN is_default;
