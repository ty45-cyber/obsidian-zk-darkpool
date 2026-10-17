// cross-chain-vaults/ethereum-erc7943/DeployObsidianVault.s.sol
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./MockMidnightSettlementVerifier.sol";
import "./MockInstitutionalERC20.sol";
import "./ObsidianERC7943Vault.sol";

interface Vm {
    function envUint(string calldata name) external view returns (uint256);
    function envOr(string calldata name, uint256 defaultValue) external view returns (uint256);
    function envOr(string calldata name, address defaultValue) external view returns (address);
    function addr(uint256 privateKey) external pure returns (address);
    function startBroadcast(uint256 privateKey) external;
    function stopBroadcast() external;
}

/**
 * @title DeployObsidianVault
 * @notice Zero-dependency Foundry deployment script for Sepolia / Anvil.
 *         Deploys MockMidnightSettlementVerifier, Mock BUIDL (margin bond token),
 *         Mock Centrifuge Senior Note (ERC-7943 RWA token), and ObsidianERC7943Vault.
 *
 * Usage:
 *   forge script DeployObsidianVault.s.sol:DeployObsidianVault \
 *     --rpc-url $SEPOLIA_RPC_URL --broadcast -vvvv
 */
contract DeployObsidianVault {
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    // Default Anvil account #0 private key for local dry-runs when PRIVATE_KEY is unset
    uint256 internal constant DEFAULT_ANVIL_PK =
        0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;

    function run()
        external
        returns (
            address verifierAddr,
            address marginBondTokenAddr,
            address rwaAssetTokenAddr,
            address vaultAddr
        )
    {
        uint256 deployerPk = vm.envOr("PRIVATE_KEY", DEFAULT_ANVIL_PK);
        address deployer = vm.addr(deployerPk);

        address existingVerifier = vm.envOr("MIDNIGHT_VERIFIER_ADDRESS", address(0));
        address existingMarginToken = vm.envOr("MARGIN_BOND_TOKEN_ADDRESS", address(0));
        address protocolReserve = vm.envOr("PROTOCOL_RESERVE_ADDRESS", deployer);

        vm.startBroadcast(deployerPk);

        if (existingVerifier == address(0)) {
            MockMidnightSettlementVerifier verifier = new MockMidnightSettlementVerifier();
            verifierAddr = address(verifier);
        } else {
            verifierAddr = existingVerifier;
        }

        if (existingMarginToken == address(0)) {
            MockInstitutionalERC20 marginToken = new MockInstitutionalERC20(
                "BlackRock USD Institutional Digital Liquidity Fund (Mock)",
                "mBUIDL",
                10_000_000 * 1e18,
                deployer
            );
            marginBondTokenAddr = address(marginToken);
        } else {
            marginBondTokenAddr = existingMarginToken;
        }

        MockInstitutionalERC20 rwaToken = new MockInstitutionalERC20(
            "Centrifuge Senior Credit Note ERC-7943 (Mock)",
            "mCSN",
            50_000_000 * 1e18,
            deployer
        );
        rwaAssetTokenAddr = address(rwaToken);

        ObsidianERC7943Vault vault = new ObsidianERC7943Vault(
            verifierAddr,
            marginBondTokenAddr,
            protocolReserve
        );
        vaultAddr = address(vault);

        vm.stopBroadcast();
    }
}
