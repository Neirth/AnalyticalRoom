# AnalyticalRoom

![AnalyticalRoom](https://placehold.co/800x200/34b1eb/000000?text=Analytical+Room&font=raleway)

## Project Description

**AnalyticalRoom** is a comprehensive monorepo of specialized MCP (Model Context Protocol) servers designed for deep analytics and logical reasoning. This system provides a suite of advanced tools for complex data analysis, logical inference, and structured information processing, engineered to integrate seamlessly with Claude and other language models.

The project serves as a unified platform for analytical capabilities, offering multiple specialized MCP servers that can work independently or in conjunction to provide sophisticated reasoning and analysis features.

## Key Features

- **Modular Architecture**: Monorepo structure with specialized, independent MCP servers
- **High Performance**: Implemented in Rust for maximum efficiency and reliability
- **Secure Authentication**: OAuth 2.0 integration for MCP session security
- **Persistent Storage**: SurrealDB integration for structured data persistence
- **Protocol Compatibility**: Full MCP specification compliance for seamless model integration

## Technologies Used

### Core Framework
- **Rust** - Primary language for performance-critical components
- **Tokio** - Asynchronous runtime for concurrent operations
- **Axum** - Modern web framework for HTTP services
- **SurrealDB** - Multi-model database for data persistence
- **Nemo** - A Inference engine for logical reasoning

### Communication & Protocols
- **MCP (Model Context Protocol)** - Primary interface for model communication
- **OAuth 2.0** - Authentication and authorization framework
- **HTTP/WebSocket** - Transport protocols for data exchange
- **JSON-RPC** - Structured communication protocol

### Analytics & Logic
- **Datalog** - Declarative logic programming language
- **Inference Engine** - Automated reasoning system
- **Knowledge Base** - Structured premise management

## Requirements

Before installing AnalyticalRoom, ensure you have:

- **Rust** 1.80.0 or higher
- **Cargo** (included with Rust installation)
- **Git** for repository cloning

## Installation

### Clone the Repository
```bash
git clone https://github.com/username/AnalyticalRoom.git
cd AnalyticalRoom
```

### Build the Project
```bash
# Build all packages in release mode
cargo build --release

# Run comprehensive test suite
cargo test

# Build specific packages
cargo build -p deep_analytics --release
cargo build -p logical_engine --release
```

### Run the Services
```bash
# Start Deep Analytics MCP Server
cargo run -p deep_analytics

# Start Logical Engine MCP Server (coming soon)
cargo run -p logical_engine
```

## Project Structure

```
AnalyticalRoom/
├── packages/
│   ├── deep_analytics/              # Deep Analytics MCP Server
│   │   ├── src/
│   │   │   ├── controllers/         # HTTP and MCP request handlers
│   │   │   │   ├── mcp_controller.rs
│   │   │   │   ├── auth_controller.rs
│   │   │   │   └── health_controller.rs
│   │   │   ├── models/              # Data models and structures
│   │   │   ├── services/            # Business logic implementation
│   │   │   └── main.rs              # Application entry point
│   │   ├── tests/                   # Integration tests
│   │   ├── .docker/                 # Container configuration
│   │   └── Cargo.toml
│   │
│   └── logical_engine/              # Logical Inference MCP Server
│       ├── src/
│       │   ├── controllers/         # HTTP and MCP request handlers
│       │   │   ├── mcp_controller.rs
│       │   │   ├── auth_controller.rs
│       │   │   └── health_controller.rs
│       │   ├── models/              # Data models and structures
│       │   ├── services/            # Business logic implementation
│       │   └── main.rs              # Application entry point
│       ├── tests/                   # Integration tests
│       ├── .docker/                 # Container configuration
│       └── Cargo.toml
│
├── utils/                           # Shared utilities and common code
├── docs/                            # Project documentation
├── .github/
│   └── workflows/                   # CI/CD pipeline definitions
├── target/                          # Build artifacts (generated)
├── Cargo.toml                       # Workspace configuration
├── Cargo.lock                       # Dependency lock file
└── README.md
```

## Detailed Capabilities

### Deep Analytics MCP Server

The `deep_analytics` package provides comprehensive analytical capabilities:

**Core Features:**
- Advanced data structure analysis and processing
- SurrealDB integration for efficient data storage and querying
- RESTful API endpoints alongside MCP protocol support
- OAuth 2.0 authentication system for secure access
- Time-series analysis and temporal data processing
- Statistical analysis and pattern recognition

**Technical Implementation:**
- Asynchronous request handling with Tokio
- Modular controller architecture for scalability
- Comprehensive error handling and logging
- Memory-efficient data processing algorithms

### Logical Engine MCP Server

The `logical_engine` package provides Datalog-based logical inference using the Nemo reasoning engine:

**Core Features:**
- Datalog-based inference engine for logical reasoning (using Nemo v0.8.1-dev)
- Dynamic knowledge base construction and management
- Custom rule and fact definition in Datalog syntax
- Automated conclusion derivation from premises
- Direct integration with language models for collaborative reasoning
- Comprehensive syntax and semantic validation
- Session-isolated knowledge bases for multi-user support

**Technical Implementation:**
- Nemo engine integration via spawn_blocking for async compatibility
- MCP protocol support with tool-based API
- Regex-based syntax validation with semantic checks
- Atomic and non-atomic bulk operations
- Timeout protection for long-running inferences
- OAuth 2.0 dummy authentication for MCP compatibility

**Nemo Integration:**
- Version: main branch (commit 8d8aaae)
- Repository: https://github.com/knowsys/nemo
- Documentation: https://knowsys.github.io/nemo-doc/
- Requires: Rust nightly toolchain

### System Architecture

**Design Principles:**
- **Modularity**: Each MCP server operates independently with clear interfaces
- **Scalability**: Asynchronous architecture supporting concurrent operations
- **Interoperability**: Standard protocol compliance for broad compatibility
- **Extensibility**: Plugin-based architecture for easy expansion
- **Reliability**: Comprehensive error handling and recovery mechanisms

## Development Configuration

### Environment Variables
```bash
export BIND_ADDRESS="0.0.0.0:8080"
export DATABASE_URL="memory"
export RUST_LOG="info"
```

### Development Workflow
```bash
# Run with auto-reload during development
cargo watch -x "run -p deep_analytics"

# Enable detailed logging
RUST_LOG=debug cargo run -p deep_analytics

# Run specific test suites
cargo test -p deep_analytics
cargo test -p logical_engine
```

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE.md) file for complete details.