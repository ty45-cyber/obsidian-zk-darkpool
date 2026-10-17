// cross-chain-vaults/ethereum-erc7943/MockInstitutionalERC20.sol
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./ObsidianERC7943Vault.sol";

/**
 * @title MockInstitutionalERC20
 * @notice Minimal ERC-20 / ERC-7943 token implementation for Sepolia testnet deployment
 *         (used for Mock BlackRock BUIDL margin bond token and Mock Centrifuge RWA principal token).
 */
contract MockInstitutionalERC20 is IERC20 {
    string public name;
    string public symbol;
    uint8 public constant decimals = 18;
    uint256 public totalSupply;
    address public immutable issuer;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);

    constructor(string memory _name, string memory _symbol, uint256 _initialSupply, address _recipient) {
        name = _name;
        symbol = _symbol;
        issuer = msg.sender;
        if (_initialSupply > 0 && _recipient != address(0)) {
            _mint(_recipient, _initialSupply);
        }
    }

    function mint(address to, uint256 amount) external {
        require(msg.sender == issuer, "Only issuer can mint");
        _mint(to, amount);
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transfer(address to, uint256 amount) external override returns (bool) {
        return _transfer(msg.sender, to, amount);
    }

    function transferFrom(address from, address to, uint256 amount) external override returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        if (allowed != type(uint256).max) {
            require(allowed >= amount, "ERC20: insufficient allowance");
            allowance[from][msg.sender] = allowed - amount;
        }
        return _transfer(from, to, amount);
    }

    function _transfer(address from, address to, uint256 amount) internal returns (bool) {
        require(to != address(0), "ERC20: transfer to zero");
        uint256 bal = balanceOf[from];
        require(bal >= amount, "ERC20: insufficient balance");
        unchecked {
            balanceOf[from] = bal - amount;
            balanceOf[to] += amount;
        }
        emit Transfer(from, to, amount);
        return true;
    }

    function _mint(address to, uint256 amount) internal {
        require(to != address(0), "ERC20: mint to zero");
        totalSupply += amount;
        unchecked {
            balanceOf[to] += amount;
        }
        emit Transfer(address(0), to, amount);
    }
}
