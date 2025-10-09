# Logical Inference MCP Server

A Datalog-based logical inference server using the [Nemo](https://github.com/knowsys/nemo) reasoning engine, exposing inference capabilities through the Model Context Protocol (MCP).

## Overview

The Logical Inference MCP Server provides automated reasoning capabilities using Datalog syntax. It allows AI models and other clients to:

- Build logical knowledge bases with facts and rules
- Execute queries to derive conclusions
- Validate Datalog syntax and semantics
- Get human-readable explanations of inferences

## Nemo Engine Integration

This package integrates with Nemo, a modern Datalog reasoning engine developed by KnowSys.

### Nemo Version

- **Repository**: https://github.com/knowsys/nemo
- **Branch**: main (latest)
- **Commit**: 8d8aaae (as of integration)
- **Documentation**: https://knowsys.github.io/nemo-doc/
- **Technical Paper**: https://ceur-ws.org/Vol-3801/short3.pdf

### Integration Approach

Due to Nemo's use of `Rc<T>` (non-thread-safe reference counting), the engine cannot be directly used in async contexts that require `Send` trait bounds. This package works around this limitation by:

1. Storing the Datalog program as text
2. Using `tokio::task::spawn_blocking` to run Nemo operations in a dedicated thread pool
3. Recreating the Nemo engine for each inference operation

While this approach has some performance overhead, it allows proper integration with the async MCP protocol while maintaining session isolation.

## Architecture

```
Nemo Engine (External Library) 
    ↓
LogicalInferenceEngine (Service - wraps Nemo with Send-safe operations)
    ↓
LogicalInferenceServer (MCP Controller - exposes tools via MCP)
```

### Key Components

- **LogicalInferenceEngine**: Core service that wraps Nemo operations
  - Manages Datalog program text
  - Validates syntax using regex patterns
  - Executes Nemo operations in blocking threads
  - Provides semantic validation (unbound variables, etc.)

- **LogicalInferenceServer**: MCP controller
  - Exposes inference tools via MCP protocol
  - Handles session isolation (each session has independent knowledge base)
  - Provides dummy OAuth authentication for compatibility

- **Models**: Data structures for inference results
  - `InferenceResult`: Query results with bindings and traces
  - `ValidateResult`: Rule validation feedback
  - `AddBulkResult`: Bulk operation results with error details

## MCP Tools

### `add_bulk`
Add multiple Datalog facts and/or rules in bulk.

**Parameters:**
- `datalog` (string): Multiple Datalog statements separated by newlines
- `atomic` (boolean): If true, all statements must be valid or none are applied

**Example:**
```json
{
  "datalog": "perro(fido).\nexiste(fido).\ncome(X) :- perro(X), existe(X).",
  "atomic": true
}
```

### `query`
Execute a logical query against the knowledge base.

**Parameters:**
- `query` (string): Datalog query starting with `?-`
- `timeout_ms` (number, optional): Maximum time in milliseconds (default: 5000)

**Example:**
```json
{
  "query": "?- come(X).",
  "timeout_ms": 3000
}
```

### `validate_rule`
Validate a Datalog rule for syntax and semantic issues.

**Parameters:**
- `rule` (string): Datalog rule to validate

**Example:**
```json
{
  "rule": "come(X) :- perro(X), existe(X)."
}
```

### `explain_inference`
Get a human-readable explanation of an inference.

**Parameters:**
- `trace_json` (object): Trace JSON from a query result
- `short` (boolean, optional): Whether to return a short summary

### `list_premises`
List all current premises (facts and rules) in the knowledge base.

### `reset`
Clear all facts and rules from the knowledge base.

## Datalog Syntax

### Facts
```prolog
perro(fido).
existe(fido).
```

### Rules
```prolog
come(X) :- perro(X), existe(X).
mortal(X) :- humano(X).
```

### Queries
```prolog
?- come(fido).
?- mortal(X).
```

### Syntax Rules
- Facts: `predicate(constant).`
- Rules: `head(X) :- body1(X), body2(X).`
- Variables start with uppercase (X, Y, Z)
- Constants are lowercase or quoted
- Comments start with `%`

## Building and Running

### Requirements

- Rust nightly toolchain (required by Nemo)
- Cargo

### Build

```bash
cd packages/logical_engine
cargo +nightly build --release
```

### Run

```bash
cargo +nightly run --release
```

The server will start on `http://localhost:8081` by default.

## Testing

Run the test suite:

```bash
cargo +nightly test
```

The tests cover:
- Syntax validation
- Bulk operations (atomic and non-atomic)
- Rule validation (unbound variables, empty rules)
- Session isolation
- Error handling

## Development Notes

### Rust Toolchain

This package requires nightly Rust due to Nemo's use of unstable features. The `rust-toolchain.toml` file in this directory ensures the correct toolchain is used.

### Send/Sync Constraints

Nemo's `Engine` type is not `Send` or `Sync` due to its use of `Rc<T>`. This package uses `spawn_blocking` to work around this limitation, allowing safe use in async contexts.

### Future Improvements

- Full query execution with variable bindings
- Trace extraction and conversion to human-readable explanations
- Support for Nemo's advanced features (stratified negation, aggregates)
- Performance optimizations (caching, incremental updates)

## License

GPL-3.0 - See LICENSE.md in the repository root.
