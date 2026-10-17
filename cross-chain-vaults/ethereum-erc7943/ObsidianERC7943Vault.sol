// cross-chain-vaults/ethereum-erc7943/ObsidianERC7943Vault.sol
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./IMidnightSettlementVerifier.sol";

// Minimal ERC20 Interface
interface IERC20 {
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

/**
 * @title ObsidianERC7943Vault
 * @notice Native Ethereum lock-and-attest vault for institutional RWA dark pool clearing.
 * @dev Enforces bridge-free atomic DvP and the 2.5% yield-bearing margin bond slashing waterfall.
 */
contract ObsidianERC7943Vault {
    IMidnightSettlementVerifier public immutable midnightVerifier;
    IERC20 public immutable marginBondToken; // e.g., BlackRock BUIDL or USYC

    uint256 public constant MARGIN_BOND_BPS = 250; // 2.5% Maintenance Margin
    uint256 public constant PENALTY_MAKE_WHOLE_BPS = 8000; // 80% to Counterparty
    uint256 public constant PENALTY_RESERVE_BPS = 2000; // 20% to Protocol Reserve

    address public immutable protocolReserve;

    struct VaultLock {
        address trader;
        address assetToken;
        uint256 assetAmount;
        uint256 marginBondAmount;
        uint64 lockTimestamp;
        bool isSettled;
    }

    mapping(bytes32 => VaultLock) public vaultLocks;
    mapping(address => uint256) public restingMarginBonds;

    event VaultLocked(bytes32 indexed lockHash, address indexed trader, address asset, uint256 amount);
    event SettlementExecuted(bytes32 indexed lockHash, bytes32 indexed midnightBatchRoot);
    event MarginSlashed(address indexed defaultingTrader, uint256 slashedAmount);
    event EscapeHatchTriggered(bytes32 indexed lockHash, address indexed trader);

    constructor(address _midnightVerifier, address _marginBondToken, address _protocolReserve) {
        require(_midnightVerifier != address(0), "Invalid verifier");
        require(_marginBondToken != address(0), "Invalid margin token");
        require(_protocolReserve != address(0), "Invalid reserve");
        
        midnightVerifier = IMidnightSettlementVerifier(_midnightVerifier);
        marginBondToken = IERC20(_marginBondToken);
        protocolReserve = _protocolReserve;
    }

    /**
     * @notice Deposits yield-bearing collateral (e.g., BUIDL) to satisfy the 2.5% gross pre-funding margin.
     */
    function depositMarginBond(uint256 amount) external {
        require(amount > 0, "Zero amount");
        require(marginBondToken.transferFrom(msg.sender, address(this), amount), "Margin transfer failed");
        restingMarginBonds[msg.sender] += amount;
    }

    /**
     * @notice Step 1: Lock RWA principal. No bridge required.
     * @param assetToken The ERC-7943 compliant RWA token (e.g., Centrifuge Senior Note).
     * @param amount The gross block size to be committed to the Midnight dark pool.
     */
    function lockVault(address assetToken, uint256 amount) external returns (bytes32 lockHash) {
        require(amount > 0, "Zero lock");
        uint256 requiredMargin = (amount * MARGIN_BOND_BPS) / 10000;
        require(restingMarginBonds[msg.sender] >= requiredMargin, "Insufficient margin bond");

        // Lock the underlying RWA principal into the escrow vault
        require(IERC20(assetToken).transferFrom(msg.sender, address(this), amount), "RWA transfer failed");

        // Compute unique lock hash
        lockHash = keccak256(abi.encodePacked(msg.sender, assetToken, amount, block.timestamp));
        
        vaultLocks[lockHash] = VaultLock({
            trader: msg.sender,
            assetToken: assetToken,
            assetAmount: amount,
            marginBondAmount: requiredMargin,
            lockTimestamp: uint64(block.timestamp),
            isSettled: false
        });

        // Reserve the margin bond so it cannot be withdrawn while the batch is active
        restingMarginBonds[msg.sender] -= requiredMargin;

        emit VaultLocked(lockHash, msg.sender, assetToken, amount);
    }

    /**
     * @notice Step 2: Atomic Delivery-versus-Payment (DvP) controlled by the Midnight settlement proof.
     */
    function executeSettlement(
        bytes32 lockHash,
        address counterparty,
        bytes32 midnightBatchRoot,
        bytes calldata midnightZkProof
    ) external {
        VaultLock storage lock = vaultLocks[lockHash];
        require(lock.assetAmount > 0, "Invalid lock");
        require(!lock.isSettled, "Already settled");

        // Cryptographically verify the Midnight Compact state transition
        bool isValid = midnightVerifier.verifyBatchSettlement(midnightZkProof, midnightBatchRoot, lockHash);
        require(isValid, "Invalid Midnight ZK Settlement Proof");

        lock.isSettled = true;
        
        // Execute Atomic DvP: Transfer the locked RWA to the designated counterparty
        require(IERC20(lock.assetToken).transfer(counterparty, lock.assetAmount), "DvP transfer failed");

        // Release the reserved margin bond back to the trader's resting balance
        restingMarginBonds[lock.trader] += lock.marginBondAmount;

        emit SettlementExecuted(lockHash, midnightBatchRoot);
    }

    /**
     * @notice The Failure Waterfall: If a desk fails to deliver cash legs on the net settlement, 
     * the performance bond is slashed. 80% to counterparty, 20% to protocol.
     */
    function slashDefaultingTrader(bytes32 lockHash, address affectedCounterparty) external {
        // In production, this requires an invalidation proof from Midnight indicating failure to deliver.
        // For the hackathon build, we simulate the modifier.
        require(msg.sender == protocolReserve, "Unauthorized slasher");
        
        VaultLock storage lock = vaultLocks[lockHash];
        require(!lock.isSettled, "Cannot slash settled lock");
        
        lock.isSettled = true; // Freeze the lock
        uint256 penalty = lock.marginBondAmount;
        
        uint256 makeWhole = (penalty * PENALTY_MAKE_WHOLE_BPS) / 10000;
        uint256 reserveFee = penalty - makeWhole;

        require(marginBondToken.transfer(affectedCounterparty, makeWhole), "Make-whole transfer failed");
        require(marginBondToken.transfer(protocolReserve, reserveFee), "Reserve transfer failed");

        // Return the locked principal back to the defaulting trader (trade is cancelled, but margin is lost)
        require(IERC20(lock.assetToken).transfer(lock.trader, lock.assetAmount), "Principal return failed");

        emit MarginSlashed(lock.trader, penalty);
    }

    /**
     * @notice 120s L1 Escape Hatch: If the AWS Nitro Enclave crashes or censors the trader, 
     * they can force an exit via a Midnight proof without Enclave assistance.
     */
    function forceUnilateralExit(bytes32 lockHash, bytes calldata escapeHatchZkProof) external {
        VaultLock storage lock = vaultLocks[lockHash];
        require(lock.trader == msg.sender, "Unauthorized exit");
        require(!lock.isSettled, "Already settled");
        require(block.timestamp > lock.lockTimestamp + 120, "120s timeout not reached");

        // Verify the `force_unilateral_exit` proof directly from Midnight
        bool isValid = midnightVerifier.verifyEscapeHatch(escapeHatchZkProof, lockHash);
        require(isValid, "Invalid Midnight Escape Hatch Proof");

        lock.isSettled = true;

        // Unlock both the principal and the reserved margin bond
        restingMarginBonds[lock.trader] += lock.marginBondAmount;
        require(IERC20(lock.assetToken).transfer(lock.trader, lock.assetAmount), "Unlock transfer failed");

        emit EscapeHatchTriggered(lockHash, lock.trader);
    }
}