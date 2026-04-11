// Package synap provides a Go client SDK for the Synap server.
// It communicates via the StreamableHTTP endpoint (POST /api/v1/command).
package synap

import "fmt"

// SynapError represents an error returned by the Synap server or the SDK.
type SynapError struct {
	// Code is a short machine-readable identifier (e.g. "server_error", "not_found").
	Code string
	// Message is a human-readable description of the error.
	Message string
}

func (e *SynapError) Error() string {
	return fmt.Sprintf("synap [%s]: %s", e.Code, e.Message)
}

// newServerError creates a SynapError with code "server_error".
func newServerError(msg string) *SynapError {
	return &SynapError{Code: "server_error", Message: msg}
}

// newInvalidResponseError creates a SynapError with code "invalid_response".
func newInvalidResponseError(msg string) *SynapError {
	return &SynapError{Code: "invalid_response", Message: msg}
}
