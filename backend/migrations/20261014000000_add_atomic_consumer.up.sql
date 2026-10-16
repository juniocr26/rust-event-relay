-- Consumer state is independent of relay lifecycle and retained indefinitely.
CREATE SCHEMA consumer;
REVOKE ALL ON SCHEMA consumer FROM PUBLIC;
CREATE TABLE consumer.stock (
 warehouse_id TEXT COLLATE "C" NOT NULL CHECK (warehouse_id ~ '^[A-Za-z0-9._-]{1,128}$'),
 sku TEXT COLLATE "C" NOT NULL CHECK (sku ~ '^[A-Za-z0-9._-]{1,128}$'),
 units BIGINT NOT NULL CHECK (units >= 0),
 PRIMARY KEY (warehouse_id, sku)
);
CREATE TABLE consumer.receipts (
 consumer_name TEXT COLLATE "C" NOT NULL CHECK (consumer_name ~ '^[A-Za-z0-9._-]{1,128}$'),
 event_id UUID NOT NULL CHECK (event_id <> '00000000-0000-0000-0000-000000000000'),
 representation_version SMALLINT NOT NULL CHECK (representation_version = 1),
 immutable_content BYTEA NOT NULL CHECK (octet_length(immutable_content) BETWEEN 1 AND 16384),
 warehouse_id TEXT COLLATE "C" NOT NULL,
 sku TEXT COLLATE "C" NOT NULL,
 quantity BIGINT NOT NULL CHECK (quantity > 0),
 stock_before BIGINT CHECK (stock_before >= 0),
 stock_after BIGINT CHECK (stock_after >= 0),
 completed_at TIMESTAMPTZ CHECK (isfinite(completed_at)),
 PRIMARY KEY (consumer_name, event_id),
 FOREIGN KEY (warehouse_id, sku) REFERENCES consumer.stock DEFERRABLE INITIALLY DEFERRED,
 CHECK ((stock_before IS NULL AND stock_after IS NULL AND completed_at IS NULL)
     OR (stock_before IS NOT NULL AND stock_after IS NOT NULL AND completed_at IS NOT NULL
         AND stock_after::numeric = stock_before::numeric + quantity::numeric))
);
-- The queued INSERT snapshot is incomplete. Inspect the final row instead.
CREATE FUNCTION consumer.require_complete_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS (SELECT 1 FROM consumer.receipts r
     WHERE r.consumer_name=NEW.consumer_name AND r.event_id=NEW.event_id
       AND r.stock_before IS NOT NULL AND r.stock_after IS NOT NULL AND r.completed_at IS NOT NULL) THEN
   RAISE EXCEPTION 'consumer receipt incomplete' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER receipt_complete AFTER INSERT OR UPDATE ON consumer.receipts
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION consumer.require_complete_receipt();
CREATE FUNCTION consumer.guard_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP <> 'UPDATE' THEN
   RAISE EXCEPTION 'consumer receipt removal forbidden' USING ERRCODE='42501';
 END IF;
 IF OLD.completed_at IS NOT NULL OR
   (NEW.consumer_name,NEW.event_id,NEW.representation_version,NEW.immutable_content,NEW.warehouse_id,NEW.sku,NEW.quantity)
   IS DISTINCT FROM
   (OLD.consumer_name,OLD.event_id,OLD.representation_version,OLD.immutable_content,OLD.warehouse_id,OLD.sku,OLD.quantity) THEN
   RAISE EXCEPTION 'consumer identity immutable' USING ERRCODE='42501';
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER receipt_guard BEFORE UPDATE OR DELETE ON consumer.receipts
 FOR EACH ROW EXECUTE FUNCTION consumer.guard_receipt();
CREATE TRIGGER receipt_no_truncate BEFORE TRUNCATE ON consumer.receipts
 FOR EACH STATEMENT EXECUTE FUNCTION consumer.guard_receipt();
REVOKE ALL ON consumer.stock, consumer.receipts FROM PUBLIC;
REVOKE ALL ON FUNCTION consumer.require_complete_receipt(), consumer.guard_receipt() FROM PUBLIC;
-- Runtime grants are deployment-owned: schema USAGE; stock SELECT/INSERT/UPDATE(units);
-- receipts SELECT/INSERT/UPDATE(stock_before,stock_after,completed_at). No DELETE/TRUNCATE/DDL.
