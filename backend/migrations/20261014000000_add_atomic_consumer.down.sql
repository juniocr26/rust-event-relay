-- Destructive: loses stock, identity retention and every historical receipt.
-- Only validate this rollback in exact disposable databases.
DROP TABLE consumer.receipts;
DROP TABLE consumer.stock;
DROP FUNCTION consumer.require_complete_receipt();
DROP FUNCTION consumer.guard_receipt();
DROP SCHEMA consumer;
