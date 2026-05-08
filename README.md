# Meridian Protocol.

Biggest gas saving in the contract: 256 nonce per storage slot. 
A trader submitting 256 fills in one block touches only 1 cold SLOAD + 1 SSTORE instead of 256.