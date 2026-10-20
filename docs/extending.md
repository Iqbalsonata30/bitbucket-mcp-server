# Adding Endpoints and Tools

Add a capability from the inside out:

1. Add or extend a domain model and invariant without REST-specific fields.
2. Add a typed domain error only if callers can act on it distinctly.
3. Define the smallest outbound application port needed by the use case.
4. Implement one application use case and unit-test it with a fake port.
5. Add private REST request/response DTOs and explicit domain conversions.
6. Implement the port in the Bitbucket adapter and test it against a mock HTTP server.
7. Add independent MCP request and response schemas.
8. Add a thin tool handler and MCP protocol-level tests.
9. Document version requirements and update capability discovery.

Do not add HTTP status handling to a tool, expose a REST DTO, or make an
application use case depend on an MCP SDK type. New optional APIs must be
isolated behind capabilities rather than scattered version checks.

Before adding an endpoint, verify it against the official REST documentation
for every claimed Bitbucket Server/Data Center version. Record the API path,
method, required permission, pagination style, idempotency, and error payloads
in adapter tests.
