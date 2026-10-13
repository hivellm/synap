## 1. Implementation
- [ ] 1.1 Add an admin-only command returning the effective server configuration with passwords, API keys and Hub keys redacted
- [ ] 1.2 Load the configuration view from that command and show load failures as errors instead of the sample configuration
- [ ] 1.3 Make the configuration view read-only with copy and download of the YAML, removing the save path that never reached the server
- [ ] 1.4 Call `key.type` and `memory.usage` from the structure inspector and parse their real responses
- [ ] 1.5 Add contract tests of the GUI API client against a real server for configuration, type, memory, missing keys, permission errors and encoded key names
- [ ] 1.6 Smoke test: create each structure through the server and inspect it in the GUI

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
