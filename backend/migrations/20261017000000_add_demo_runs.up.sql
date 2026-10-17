-- Additive only; rollout exclusively to the dedicated demo database.
CREATE SCHEMA demo;
REVOKE ALL ON SCHEMA demo FROM PUBLIC;
CREATE TABLE demo.runs (
 run_id UUID PRIMARY KEY,
 request_id UUID NOT NULL UNIQUE,
 event_id UUID NOT NULL UNIQUE REFERENCES relay.outbox_events(id) DEFERRABLE INITIALLY DEFERRED,
 warehouse_id TEXT NOT NULL UNIQUE,
 sku TEXT NOT NULL UNIQUE,
 scenario TEXT NOT NULL CHECK (scenario = 'normal'),
 initial_stock BIGINT NOT NULL CHECK (initial_stock = 10),
 quantity BIGINT NOT NULL CHECK (quantity = 5),
 accepted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY (warehouse_id,sku) REFERENCES consumer.stock DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE demo.goods_receipts (
 fact_id UUID PRIMARY KEY,
 run_id UUID NOT NULL UNIQUE REFERENCES demo.runs,
 warehouse_id TEXT NOT NULL,
 sku TEXT NOT NULL,
 quantity BIGINT NOT NULL CHECK (quantity = 5),
 recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
REVOKE ALL ON demo.runs, demo.goods_receipts FROM PUBLIC;
