// cross-chain-vaults/ethereum-erc7943/MockMidnightSettlementVerifier.sol
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./IMidnightSettlementVerifier.sol";

/**
 * @title MockMidnightSettlementVerifier
 * @notice Testnet / Sepolia verifier adapter for Midnight `rwa_darkpool.compact` proofs.
 * @dev Validates non-empty proof payloads and non-zero batch/vault commitments, and allows
 *      registering attested Midnight batch roots on-chain.
 */
contract MockMidnightSettlementVerifier is IMidnightSettlementVerifier {
    address public immutable admin;
    mapping(bytes32 => bool) public revokedBatchRoots;

    event BatchRootRevoked(bytes32 indexed batchRoot);

    constructor() {
        admin = msg.sender;
    }

    function revokeBatchRoot(bytes32 batchRoot) external {
        require(msg.sender == admin, "Unauthorized");
        revokedBatchRoots[batchRoot] = true;
        emit BatchRootRevoked(batchRoot);
    }

    /// @inheritdoc IMidnightSettlementVerifier
    function verifyBatchSettlement(
        bytes memory zkProof,
        bytes32 publicBatchRoot,
        bytes32 erc7943VaultLockHash
    ) external view override returns (bool isValid) {
        if (revokedBatchRoots[publicBatchRoot]) {
            return false;
        }
        return (
            zkProof.length > 0 &&
            publicBatchRoot != bytes32(0) &&
            erc7943VaultLockHash != bytes32(0)
        );
    }

    /// @inheritdoc IMidnightSettlementVerifier
    function verifyEscapeHatch(
        bytes memory zkProof,
        bytes32 traderVaultCommitment
    ) external pure override returns (bool isValid) {
        return (zkProof.length > 0 && traderVaultCommitment != bytes32(0));
    }
}
