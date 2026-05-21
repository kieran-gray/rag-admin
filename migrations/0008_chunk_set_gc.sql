ALTER TABLE chunk_sets
    ADD COLUMN pinned BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE indexings
    ADD CONSTRAINT indexings_chunk_set_id_fkey
    FOREIGN KEY (chunk_set_id) REFERENCES chunk_sets (chunk_set_id) ON DELETE RESTRICT;

ALTER TABLE evaluation_variant_results
    ADD CONSTRAINT evaluation_variant_results_chunk_set_id_fkey
    FOREIGN KEY (chunk_set_id) REFERENCES chunk_sets (chunk_set_id) ON DELETE RESTRICT;
