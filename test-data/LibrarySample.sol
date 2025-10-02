// SPDX-License-Identifier: MIT
pragma solidity =0.8.19;

/// @title StringUtils
library StringUtils {
    function nothing(string memory input) public pure returns (string memory) {
        return input;
    }
}

contract BasicSample {
    using StringUtils for string;
}
