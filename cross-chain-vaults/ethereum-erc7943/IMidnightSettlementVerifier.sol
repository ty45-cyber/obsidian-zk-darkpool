// cross-chain-vaults/ethereum-erc7943/IMidnightSettlementVerifier.sol
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/**
 * @title IMidnightSettlementVerifier
 * @notice Interfaces with the Midnight Network L1 settlement verifier.
 * @dev Re-verifies zero-knowledge proofs emitted by the `rwa_darkpool.compact` circuit.
 */
interface IMidnightSettlementVerifier {
    /**
     * @notice Verifies an atomic Delivery-versus-Payment (DvP) proof from Midnight.
     * @param zkProof The cryptographic SNARK proof payload from the Midnight enclave.
     * @param publicBatchRoot The publicly disclosed batch commitment root.
     * @param erc7943VaultLockHash The hash of the locked vault commitment on Ethereum.
     * @return isValid True if the ZK-SNARK is valid and the batch crossed successfully.
     */
    function verifyBatchSettlement(
        bytes memory zkProof,
        bytes32 publicBatchRoot,
        bytes32 erc7943VaultLockHash
    ) external view returns (bool isValid);

    /**
     * @notice Verifies a 120s L1 Escape Hatch receipt from Midnight.
     * @param zkProof The cryptographic SNARK proof from `force_unilateral_exit`.
     * @param traderVaultCommitment The original vault lock commitment.
     * @return isValid True if the enclave liveness timeout was reached and exit is authorized.
     */
    function verifyEscapeHatch(
        bytes memory zkProof,
        bytes32 traderVaultCommitment
    ) external view returns (bool isValid);
}