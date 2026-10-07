[Português brasileiro](../../pt-BR/adr/001-use-rust-for-the-relay.md) | [README](../../../README.md)

# ADR 001 — Use Rust for the relay

Status: accepted. Date: 2026-10-07.

## Context

The portfolio problem is distributed event delivery under failure. A long-running service will need explicit resource bounds, safe concurrent state and recoverable lifecycle behavior. This is also an opportunity to study systems programming; language collection is not the goal.

## Decision

Use stable Rust and Cargo. Ownership and borrowing make lifetimes and sharing explicit; memory safety in safe Rust reduces certain invalid-memory behaviors. Rust's concurrency model helps express safe ownership transfers, while Tokio and Axum provide an established async ecosystem for networking and process lifecycle. The initial project uses no unsafe code.

## Consequences and alternatives

Rust requires learning ownership, async cancellation and error design; compile times and ecosystem complexity are real costs. Memory safety does not prevent delivery loss, deadlocks, logical races, unbounded queues or incorrect retry semantics. Those need engineering and failure tests. Go, Java and other mature languages could also implement this service well; Rust is not automatically faster or superior. Its fit for a long-running infrastructure service and educational value motivate this choice, without unmeasured performance claims. Keep dependency and abstraction count proportional to demonstrated behavior.
