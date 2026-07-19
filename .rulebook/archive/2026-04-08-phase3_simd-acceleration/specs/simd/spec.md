# SIMD Acceleration Specification

## ADDED Requirements

### Requirement: Platform-Aware SIMD Dispatch
The system MUST select the best available SIMD implementation at runtime for the current CPU,
with automatic fallback to a portable scalar implementation when SIMD is unavailable.

#### Scenario: AVX2 path selected on capable x86_64 CPU
Given the binary runs on an x86_64 CPU with AVX2 support
When any SIMD-accelerated function is called
Then the AVX2 implementation MUST be used
And the result MUST be identical to the fallback implementation output

#### Scenario: Fallback used when SIMD unsupported
Given the binary runs on a CPU without SSE2 or AVX2
When any SIMD-accelerated function is called
Then the portable fallback implementation MUST be used
And no illegal instruction fault MUST occur

#### Scenario: simd feature disabled at compile time
Given the binary is compiled with --no-default-features
When any SIMD-accelerated function is called
Then the fallback implementation MUST be compiled in and used
And the binary MUST produce correct results

### Requirement: BITCOUNT SIMD Acceleration
The system MUST use SIMD popcount instructions for BITCOUNT operations on byte slices.

#### Scenario: BITCOUNT on 1MB bitmap is faster than scalar
Given a 1MB bitmap with random bit distribution
When BITCOUNT is called
Then the SIMD path MUST complete the operation
And the result MUST equal the scalar popcount result

### Requirement: BITOP SIMD Acceleration
The system MUST use SIMD vector operations for BITOP AND, OR, XOR, NOT.

#### Scenario: BITOP AND on two 512KB bitmaps
Given two 512KB bitmaps
When BITOP AND is called
Then the SIMD path MUST produce the same result as the scalar AND

### Requirement: HyperLogLog SIMD Acceleration
The system MUST use SIMD instructions for PFCOUNT register popcount and PFMERGE max-reduce.

#### Scenario: PFCOUNT uses SIMD popcount
Given a HyperLogLog with non-empty registers
When PFCOUNT is called
Then the register array popcount MUST use simd::popcount_slice

#### Scenario: PFMERGE uses SIMD max-reduce
Given two HyperLogLog structures
When PFMERGE is called
Then each register pair MUST be max-reduced using simd::max_reduce_u8

### Requirement: KEYS/SCAN memchr Acceleration
The system MUST use the memchr crate for byte-level pattern matching during KEYS and SCAN.

#### Scenario: Prefix scan uses memchr
Given a keyspace with 10000 keys
When SCAN is called with a prefix pattern
Then memchr::memmem MUST be used for substring matching
And all matching keys MUST be returned correctly

### Requirement: SIMD Correctness Guarantee
For every SIMD function, the output MUST be bit-for-bit identical to the fallback output
for all possible inputs.

#### Scenario: Randomized correctness check
Given random byte slices of varying lengths (0 to 1MB)
When both SIMD and fallback implementations process the same input
Then the outputs MUST be identical
