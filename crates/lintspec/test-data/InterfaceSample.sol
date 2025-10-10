// SPDX-License-Identifier: MIT
pragma solidity =0.8.19;

/// @title The interface
interface IInterfacedSample {
    /**
     * @notice Greets the caller
     *
     * @return _balance  Current token balance of the caller
     */
    function greet() external view returns (string memory _greeting, uint256 _balance);
}

contract InterfacedSample is IInterfacedSample {
    /// @dev some dev thingy
    function greet() external view returns (string memory _greeting, uint256 _balance) {}
}
