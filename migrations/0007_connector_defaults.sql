ALTER TABLE connectors
    ADD COLUMN default_pipeline_configuration_id UUID NULL,
    ADD COLUMN default_chunking_configuration_id UUID NULL;
