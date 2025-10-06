# Logical Engine Tests

This directory contains comprehensive tests for the logical_engine package.

## Running Tests

### Run all tests
```bash
cargo test
```

### Run only unit tests
```bash
cargo test --lib
```

### Run only integration tests
```bash
cargo test --test integration
```

### Run with output
```bash
cargo test -- --nocapture
```

### Run specific test
```bash
cargo test test_query_execution_simple
```

## Test Organization

### Unit Tests (`src/domain/services/logical_inference_engine.rs`)
Located at the bottom of the service file, these tests cover:
- Individual function behavior
- Edge cases
- Error conditions
- Validation logic

**Categories:**
- Core functionality (12 tests)
- Bulk operations (7 tests)
- Query execution (10 tests)
- Validation (8 tests)
- Materialization (4 tests)
- Edge cases (11 tests)
- Explanations (4 tests)
- Complex workflows (2 tests)

### Integration Tests (`tests/integration.rs`)
These tests verify end-to-end workflows and complex scenarios:
- Complete workflows
- Complex knowledge bases
- Multi-step operations
- Error recovery
- System integration

**Categories:**
- Basic workflows (5 tests)
- Complex analysis (7 tests)
- Query & materialization (3 tests)
- Validation workflows (4 tests)
- Edge cases (8 tests)
- Annotations (2 tests)
- Concurrent operations (3 tests)

## Test Coverage

See [TEST_COVERAGE.md](TEST_COVERAGE.md) for detailed coverage information.

**Summary:**
- 57 unit tests
- 34 integration tests
- 2 doc tests
- **93 total tests**
- All tests passing
- Zero compiler warnings

## Testing Philosophy

### 1. Comprehensive Coverage
Every public API function has multiple tests covering:
- Happy path (normal operation)
- Edge cases (boundary conditions)
- Error cases (invalid inputs)
- Integration scenarios

### 2. Edge Case Focus
Special attention to:
- Empty inputs
- Very large inputs
- Invalid syntax
- Timeout scenarios
- Concurrent access
- Special characters

### 3. Realistic Scenarios
Integration tests use realistic Datalog programs:
- Family relationships
- Animal taxonomies
- Recursive rules
- Transitive closures

### 4. Performance Validation
Stress tests verify the system handles:
- 100+ facts
- 20-level recursion
- Multiple concurrent engines
- Large rule sets

## Adding New Tests

### For New Features
1. Add unit tests in the service file's test module
2. Add integration test in `tests/integration.rs`
3. Update `TEST_COVERAGE.md`
4. Ensure all tests pass: `cargo test`

### Test Naming Convention
- Unit tests: `test_<feature>_<scenario>`
  - Example: `test_query_execution_with_variables`
- Integration tests: `test_integration_<workflow>`
  - Example: `test_integration_complex_queries`

### Test Structure
```rust
#[tokio::test]
async fn test_descriptive_name() {
    // Setup
    let mut engine = LogicalInferenceEngine::new();
    
    // Execute
    let result = engine.some_operation().await;
    
    // Verify
    assert!(result.is_ok());
    assert_eq!(expected, actual);
}
```

## Common Test Patterns

### Testing Queries
```rust
engine.load_fact("perro(fido).").await.ok();
let result = engine.query("?- perro(fido).", 5000).await;
assert!(result.proven);
```

### Testing Bulk Operations
```rust
let datalog = "fact1(a).\nfact2(b).";
let result = engine.load_bulk(datalog, true).await;
assert_eq!(result.added_count, 2);
```

### Testing Validation
```rust
let validation = engine.validate_rule("head(X) :- body(X).");
assert!(validation.is_valid);
assert!(validation.errors.is_empty());
```

### Testing Edge Cases
```rust
let result = engine.validate_datalog_syntax("");
assert!(result.is_err());
```

## Debugging Failed Tests

### View test output
```bash
cargo test test_name -- --nocapture
```

### Run with backtrace
```bash
RUST_BACKTRACE=1 cargo test test_name
```

### Check specific assertion
```bash
cargo test test_name -- --show-output
```

## Continuous Integration

All tests must pass before merging:
- Unit tests must pass
- Integration tests must pass
- No compiler warnings
- Code coverage maintained

## Performance Considerations

Some tests may take longer:
- `test_large_program_text` - Tests 200 facts
- `test_integration_stress_large_kb` - Tests 100 facts
- `test_materialize_*` - Runs Nemo inference engine

Expected test suite runtime: ~30-35 seconds

## Known Limitations

1. Query execution uses simplified heuristics (not full Nemo query API)
2. Binding extraction not yet implemented
3. Trace JSON generation is a placeholder
4. Some Nemo concurrency limitations

These are acceptable trade-offs for the current implementation scope.
