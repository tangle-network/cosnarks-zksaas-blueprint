// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import "tnt-core/BlueprintServiceManagerBase.sol";
import {IVerifier} from "./IVerifier.sol";

/// @title CoSNARKsZkSaaSBlueprintServiceManager
/// @author Tangle Network Team
/// @notice Manages the CoSNARKs zkSaaS Blueprint instance, handling circuit
/// registration metadata (verification keys) and verifying proof generation jobs.
contract CoSNARKsZkSaaSBlueprintServiceManager is BlueprintServiceManagerBase {
    // --- Constants ---

    // Job IDs (must match Rust constants)
    uint64 public constant REGISTER_CIRCUIT_JOB_ID = 0;
    uint64 public constant GENERATE_PROOF_JOB_ID = 1;

    // --- Storage ---

    /// @notice Maps circuit ID (bytes32 representation) to its verification key info.
    mapping(bytes32 => VerificationKeyInfo) public verificationKeys;

    /// @notice Information needed to verify proofs for a specific circuit.
    struct VerificationKeyInfo {
        address verifier; // Address of the specific IVerifier contract (e.g., Groth16Verifier)
        bytes verificationKey; // The actual verification key bytes
        bool exists; // Flag to check if the circuit ID is registered
    }

    // --- Events ---
    event CircuitRegistered(
        bytes32 indexed circuitId,
        address verifier,
        uint256 vkLength
    );
    event ProofVerified(
        bytes32 indexed circuitId,
        uint64 indexed jobCallId,
        bool success
    );

    // --- Constructor ---
    // Inherits constructor logic from RootChainEnabled via BlueprintServiceManagerBase

    // --- Lifecycle Hooks ---

    /// @inheritdoc IBlueprintServiceManager
    /// @dev Stores the verification key when a circuit registration job completes successfully.
    /// The result format needs careful definition between Rust and Solidity.
    /// Expected result from REGISTER_CIRCUIT_JOB_ID:
    /// - Field 0: circuitId (bytes32 or string -> bytes32)
    /// - Field 1: verifierAddress (address) - Address of the deployed IVerifier contract
    /// - Field 2: verificationKey (bytes)
    function onJobResult(
        uint64 serviceId,
        uint8 job,
        uint64 jobCallId,
        ServiceOperators.OperatorPreferences calldata operator,
        bytes calldata inputs,
        bytes calldata outputs
    )
        external
        payable
        override
        onlyFromMaster // Ensures only the Master MBSM can call this
    {
        if (job == REGISTER_CIRCUIT_JOB_ID) {
            _handleCircuitRegistrationResult(jobCallId, outputs);
        } else if (job == GENERATE_PROOF_JOB_ID) {
            _handleProofGenerationResult(serviceId, jobCallId, outputs);
        } else {
            // Handle other potential future job results or ignore
        }
    }

    // --- Internal Result Handlers ---

    function _handleCircuitRegistrationResult(
        uint64 jobCallId,
        bytes calldata resultData
    ) internal {
        // Decode the result fields (adjust indices and types as needed)
        // (bytes memory circuitIdBytes, address verifierAddress, bytes memory vkBytes) =
        //     Codec.decodeResult(resultData, (Field.Bytes, Field.Address, Field.Bytes));

        (
            bytes memory circuitIdBytes,
            address verifierAddress,
            bytes memory vkBytes
        ) = abi.decode(resultData, (bytes, address, bytes));
        bytes32 circuitId = bytes32(circuitIdBytes); // Assuming ID fits in bytes32, otherwise hash

        require(
            verifierAddress != address(0),
            "Verifier address cannot be zero"
        );
        require(vkBytes.length > 0, "Verification key cannot be empty");
        // Potentially add check: require(!verificationKeys[circuitId].exists, "Circuit already registered");

        verificationKeys[circuitId] = VerificationKeyInfo({
            verifier: verifierAddress,
            verificationKey: vkBytes,
            exists: true
        });

        emit CircuitRegistered(circuitId, verifierAddress, vkBytes.length);
        // Consider adding jobCallId to the event if useful
    }

    function _handleProofGenerationResult(
        uint64 serviceId,
        uint64 jobCallId,
        bytes calldata resultData
    ) internal {
        // Decode the result fields for GENERATE_PROOF_JOB_ID
        // Expected result format:
        // - Field 0: circuitId (bytes32) - ID of the circuit proof was for
        // - Field 1: proofBytes (bytes) - The actual ZK proof
        // - Field 2: publicInputs (bytes[]) - Array of public input bytes strings
        // (
        //     bytes memory circuitIdBytes,
        //     bytes memory proofBytes,
        //     bytes[] memory publicInputBytes
        // ) = Codec.decodeResult(
        //         resultData,
        //         (Field.Bytes, Field.Bytes, Field.BytesArray)
        //     );

        (
            bytes memory circuitIdBytes,
            bytes memory proofBytes,
            bytes[] memory publicInputBytes
        ) = abi.decode(resultData, (bytes, bytes, bytes[]));
        bytes32 circuitId = bytes32(circuitIdBytes);

        VerificationKeyInfo storage vkInfo = verificationKeys[circuitId];
        require(vkInfo.exists, "Circuit not registered");

        // Call the appropriate verifier
        IVerifier verifier = IVerifier(vkInfo.verifier);
        bool success = verifier.verifyProof(
            vkInfo.verificationKey,
            proofBytes,
            publicInputBytes
        );

        emit ProofVerified(circuitId, jobCallId, success);

        // Optional: Revert if verification fails, depending on desired behavior
        // require(success, "Proof verification failed");

        // Optional: Add logic here based on success/failure (e.g., reward operator, penalize)
        // This often interacts with staking or payment mechanisms managed by the Master MBSM or RootChain.
    }

    // --- Other Hooks (Optional Overrides) ---

    /// @inheritdoc IBlueprintServiceManager
    /// @dev Example: could enforce specific inputs for job calls if needed.
    function onJobCall(
        uint64 serviceId,
        uint8 job,
        uint64 jobCallId,
        bytes calldata inputs
    ) external payable override onlyFromMaster {
        // Optional: Add validation logic for inputs based on the job ID
        if (job == REGISTER_CIRCUIT_JOB_ID) {
            // Validate register inputs
        } else if (job == GENERATE_PROOF_JOB_ID) {
            // Validate proof generation inputs
        }
        // ... rest of logic or simply rely on base implementation (if any)
    }

    // --- View Functions (Optional) ---

    /// @notice Get the verification key info for a circuit.
    function getVerificationKeyInfo(
        bytes32 circuitId
    ) external view returns (VerificationKeyInfo memory) {
        return verificationKeys[circuitId];
    }
}
